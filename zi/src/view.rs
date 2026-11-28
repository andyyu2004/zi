use std::cell::Cell;

use rel::Relation;
use tui::LineNumberStyle;
use ustr::Ustr;
use zi_core::{Offset, Size, ViewGroupId, ViewId};
use zi_text::grapheme::{floor_grapheme_boundary, next_grapheme_boundary, prev_grapheme_boundary};
use zi_text::{self, Text as _, TextSlice};

use crate::buffer::Buffer;
use crate::config::Setting;
use crate::editor::Selector;
use crate::{BufferId, ByteCol, Direction, DisplayCol, Editor, JumpList, Location, Mode, Point};

/// View-local configuration
#[derive(Clone, Debug)]
pub struct Settings {
    /// The width of the line numbers column including a space between the number and the text
    pub line_number_width: Setting<u8>,
    pub line_number_style: Setting<LineNumberStyle>,
    /// The number of lines `<C-d>` and `<C-u>` scroll, vim's 'scroll'. 0 is half the view height.
    pub scroll: Setting<usize>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            line_number_width: Setting::new(4),
            line_number_style: Setting::new(LineNumberStyle::Absolute),
            scroll: Setting::new(0),
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SetCursorFlags: u8 {
        /// Shift the cursor right to the first non-whitespace character if necessary.
        const START_OF_LINE = 1 << 0;
        /// Do not update the cursor target column even if the cursor did not move.
        const NO_FORCE_UPDATE_TARGET = 1 << 1;
        /// Take the cursor column from the target column, then update the target as usual.
        const FROM_TARGET_COLUMN = 1 << 2;
        /// Use the target column as the cursor column and keep it as the target.
        const USE_TARGET_COLUMN =
            Self::FROM_TARGET_COLUMN.bits() | Self::NO_FORCE_UPDATE_TARGET.bits();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}

impl Selector<Self> for ViewId {
    fn select(&self, _: &Editor) -> Self {
        *self
    }
}

impl Selector<BufferId> for ViewId {
    fn select(&self, editor: &Editor) -> BufferId {
        editor.view(*self).buffer()
    }
}

/// A view is a viewport into a buffer.
/// Its buffer and group are columns of its row in the editor's [`crate::editor::db::View`].
#[derive(Debug, Clone)]
pub struct View {
    id: ViewId,
    /// The offset of the view in the buffer.
    /// i.e. this changes on scroll.
    offset: Offset,
    /// The cursor position in the buffer
    cursor: Cursor,
    jumps: JumpList<Location>,
    settings: Settings,

    /// The actual width of the line numbers column including a space between the number and the text.
    /// This should be at least `config.line_number_width` but can be larger if the line numbers are wider.
    /// This value should be updated when rendering the view.
    pub(crate) number_width: Cell<u16>,
}

impl Selector<ViewId> for View {
    #[inline]
    fn select(&self, _editor: &Editor) -> ViewId {
        self.id()
    }
}

/// A view group is a collection of views that are displayed together,
/// closing one view in the group closes all views in the group.
/// A view can be in at most one group.
#[derive(Debug, Clone, Relation)]
#[relation(key = ViewGroupId)]
pub struct ViewGroup {
    id: ViewGroupId,
    #[index(unique)]
    key: Ustr,
}

impl ViewGroup {
    pub(crate) fn new(id: ViewGroupId, key: Ustr) -> Self {
        Self { id, key }
    }

    #[inline]
    pub fn id(&self) -> ViewGroupId {
        self.id
    }

    #[inline]
    pub fn key(&self) -> Ustr {
        self.key
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Cursor {
    point: Point,
    // When we move the cursor down we may go to a shorter line, virtual column stores the column
    // that the cursor should really be at, but can't be because the line is too short.
    target_col: DisplayCol,
}

impl View {
    #[inline]
    pub fn id(&self) -> ViewId {
        self.id
    }

    #[inline]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    #[inline]
    pub fn cursor(&self) -> Point {
        self.cursor.point
    }

    pub(crate) fn target_col(&self) -> DisplayCol {
        self.cursor.target_col
    }

    pub(crate) fn set_target_col(&mut self, col: DisplayCol) {
        self.cursor.target_col = col;
    }

    /// Move the cursor and scroll back to the start, for when the view's buffer changes.
    #[inline]
    pub(crate) fn reset_position(&mut self) {
        self.cursor = Cursor::default();
        self.offset = Offset::default();
    }

    pub(crate) fn align(&mut self, size: impl Into<Size>, alignment: VerticalAlignment) {
        let cursor = self.cursor();
        let size = size.into();

        let line = match alignment {
            VerticalAlignment::Top => cursor.line(),
            VerticalAlignment::Center => cursor.line().saturating_sub(size.height as usize / 2),
            VerticalAlignment::Bottom => {
                cursor.line().saturating_sub(size.height.saturating_sub(1) as usize)
            }
        };

        self.offset = Offset::new(line, DisplayCol::default());
    }

    /// Returns the cursor coordinates in the buffer in cells (not characters) relative to the viewport.
    /// For example, '\t' is one character but is 4 cells wide (by default).
    #[inline]
    pub(crate) fn cursor_viewport_coords(&self, buf: &Buffer) -> (u16, u16) {
        assert!(
            self.offset.line <= self.cursor.point.line(),
            "cursor is above the viewport: offset={} cursor={}",
            self.offset,
            self.cursor.point,
        );
        let line_idx = self.cursor.point.line();
        let col = buf.display_col(self.cursor.point);
        assert!(
            self.offset.col <= col,
            "cursor is to the left of the viewport: offset={} col={col:?}",
            self.offset,
        );
        let x = col.cells() - self.offset.col.cells();
        let y = line_idx - self.offset.line;
        (x.try_into().unwrap(), y.try_into().unwrap())
    }

    /// `amt` is measured in graphemes or lines depending on the direction.
    pub(crate) fn move_cursor(
        &mut self,
        mode: Mode,
        size: impl Into<Size>,
        buf: &Buffer,
        direction: Direction,
        amt: usize,
    ) -> Point {
        let point = self.cursor.point;
        let pos = match direction {
            Direction::Left => {
                let Some(line) = buf.text().line(point.line()) else { return point };
                point.with_byte_col(prev_grapheme_boundary(&line.to_cow(), point.byte_col(), amt))
            }
            Direction::Right => {
                let Some(line) = buf.text().line(point.line()) else { return point };
                point.with_byte_col(next_grapheme_boundary(&line.to_cow(), point.byte_col(), amt))
            }
            // Horizontal movements set the target column.
            // Vertical movements try to keep moving to the target column.
            Direction::Up => self.cursor.point.up(amt),
            Direction::Down => self.cursor.point.down(amt),
        };

        let flags = match direction {
            Direction::Up | Direction::Down => SetCursorFlags::USE_TARGET_COLUMN,
            Direction::Left | Direction::Right => SetCursorFlags::empty(),
        };

        self.set_cursor_linewise(mode, size, buf, pos, flags)
    }

    // HACK clean this up and try not have two different implementations for a cursor move.
    // This is handling the case where motions may return an exclusive endpoint that is out of bounds.
    // We want to instead move to the last non-newline character in the text.
    pub(crate) fn set_cursor_bytewise(
        &mut self,
        mode: Mode,
        size: impl Into<Size>,
        buf: &Buffer,
        mut byte: usize,
        flags: SetCursorFlags,
    ) -> Point {
        assert!(
            !flags.contains(SetCursorFlags::FROM_TARGET_COLUMN),
            "not applicable for bytewise cursor movement"
        );

        #[expect(clippy::todo, reason = "no bytewise motion sets START_OF_LINE yet")]
        if flags.contains(SetCursorFlags::START_OF_LINE) {
            todo!()
        }

        let text = buf.text();
        let len = text.len_bytes();
        assert!(byte <= len);

        let insert = matches!(mode, Mode::Insert);

        // Ensure the cursor is in a valid position.
        let mut chars = if byte == len {
            let mut chars = text.byte_slice(..).chars().rev().peekable();
            if !insert {
                // move cursor back if not in insert mode
                byte -= chars.next().map_or(0, char::len_utf8);
            }
            chars
        } else {
            text.byte_slice(..byte).chars().rev().peekable()
        };

        // Prevent cursor being past end of line
        let prev = chars.peek().copied().filter(|&c| c != '\n');
        let pos = match prev {
            Some(prev) if !insert && text.char_at_byte(byte) == Some('\n') => {
                let pos = text.byte_to_point(byte - prev.len_utf8());
                let line = text.line(pos.line()).map_or_default(|line| line.to_cow());
                pos.with_byte_col(floor_grapheme_boundary(&line, pos.byte_col()))
            }
            Some(_) | None => text.byte_to_point(byte),
        };
        let col = buf.display_col(pos);

        if self.cursor.point != pos && self.cursor.target_col != col
            || !flags.contains(SetCursorFlags::NO_FORCE_UPDATE_TARGET)
        {
            self.cursor.target_col = col;
        }
        self.cursor.point = pos;

        self.ensure_scroll_in_bounds(size);
        #[cfg(debug_assertions)]
        std::hint::black_box(text.byte_slice(text.point_to_byte(self.cursor.point)..));
        self.cursor.point
    }

    #[inline]
    pub(crate) fn set_cursor_linewise(
        &mut self,
        mode: Mode,
        size: impl Into<Size>,
        buf: &Buffer,
        pos: Point,
        flags: SetCursorFlags,
    ) -> Point {
        let text = buf.text();
        let size = size.into();

        // Check line is in-bounds
        let mut line_idx = pos.line();
        let line = match text.line(line_idx) {
            Some(line) => line,
            _ => {
                line_idx = text.len_lines().saturating_sub(1);
                text.line(line_idx).unwrap_or_else(|| Box::new(""))
            }
        };

        let line = line.to_cow();
        let line_len = line.len();

        let (pos, requested_col) = if flags.contains(SetCursorFlags::FROM_TARGET_COLUMN) {
            let target_col = self.cursor.target_col;
            (Point::new(line_idx, buf.line_byte_col(&line, target_col)), target_col)
        } else {
            let pos = Point::new(line_idx, pos.byte_col());
            (pos, buf.line_display_col(&line, pos.byte_col()))
        };

        // Insert and visual modes may put the cursor one past the end of the line, as nvim does.
        let k = match mode {
            Mode::Insert | Mode::Visual | Mode::VisualLine | Mode::VisualBlock => 0,
            Mode::Normal | Mode::Command | Mode::OperatorPending(..) | Mode::ReplacePending => {
                line_len - prev_grapheme_boundary(&line, line_len, 1)
            }
        };

        let max_col = ByteCol::from(line_len.saturating_sub(k));

        // check column is in-bounds for the line
        let new_cursor = match pos.byte_col() {
            i if i < line_len => {
                if flags.contains(SetCursorFlags::START_OF_LINE) {
                    let mut col = 0;
                    let mut found_non_whitespace = false;
                    for c in line.chars() {
                        if !c.is_whitespace() {
                            found_non_whitespace = true;
                            break;
                        }
                        col += c.len_utf8();
                    }

                    // don't advance the cursor if the line is all spaces unless in insert mode
                    if !found_non_whitespace && !matches!(mode, Mode::Insert) {
                        col = 0;
                    }

                    pos.with_byte_col(col.max(i))
                } else {
                    pos.with_byte_col(max_col.min(i))
                }
            }
            // Cursor is out of bounds for the line, but the line exists.
            // We move the cursor to the line to the rightmost character.
            _ => pos.with_byte_col(max_col),
        };

        let new_cursor =
            new_cursor.with_byte_col(floor_grapheme_boundary(&line, new_cursor.byte_col()));

        if self.cursor.target_col != requested_col && self.cursor.point != new_cursor
            || !flags.contains(SetCursorFlags::NO_FORCE_UPDATE_TARGET)
        {
            self.cursor.target_col = buf.line_display_col(&line, new_cursor.byte_col());
        }

        self.cursor.point = new_cursor;

        // Assert that the cursor is in valid byte position. This will panic if the cursor is in
        // the middle of a code point.
        #[cfg(debug_assertions)]
        std::hint::black_box(text.byte_slice(text.point_to_byte(self.cursor.point)..));

        self.ensure_scroll_in_bounds(size);

        self.cursor.point
    }

    fn ensure_scroll_in_bounds(&mut self, size: impl Into<Size>) {
        let size = size.into();
        // Scroll the view if the cursor moves out of bounds
        if self.cursor.point.line() < self.offset.line {
            self.offset.line = self.cursor.point.line();
        } else if self.cursor.point.line() >= self.offset.line + size.height as usize {
            self.offset.line = self.cursor.point.line() - size.height as usize + 1;
        }
    }

    pub(crate) fn scroll(
        &mut self,
        mode: Mode,
        size: impl Into<Size>,
        buf: &Buffer,
        direction: Direction,
        amt: usize,
    ) {
        let size = size.into();
        let prev = self.offset;
        // don't need to bounds check the scroll, moving the cursor handles that
        match direction {
            Direction::Up => {
                self.offset.line = self.offset.line.saturating_sub(amt);
                self.move_cursor(mode, size, buf, direction, prev.line - self.offset.line);
            }
            Direction::Down => {
                self.offset.line = self.offset.line.saturating_add(amt);
                self.move_cursor(mode, size, buf, direction, self.offset.line - prev.line);
            }
            Direction::Left => {
                let col = DisplayCol::new(prev.col.cells().saturating_sub(amt));
                self.scroll_to_col(mode, size, buf, col);
            }
            Direction::Right => {
                let col = DisplayCol::new(prev.col.cells().saturating_add(amt));
                self.scroll_to_col(mode, size, buf, col);
            }
        }
        assert!(
            self.cursor.point.line() >= self.offset.line
                && self.cursor.point.line() < self.offset.line + size.height as usize,
            "cursor is out of bounds: cursor={} offset={} size={size}",
            self.cursor.point,
            self.offset,
        );
    }

    fn scroll_to_col(&mut self, mode: Mode, size: Size, buf: &Buffer, col: DisplayCol) {
        let prev = self.offset.col;
        self.offset.col = col;

        let point = self.cursor.point;
        let line = buf.text().line(point.line()).unwrap_or_else(|| Box::new(""));
        let line = line.to_cow();
        let cursor_col = buf.line_display_col(&line, point.byte_col()).cells();
        let target = DisplayCol::new((cursor_col + col.cells()).saturating_sub(prev.cells()));
        let byte_col = buf.line_byte_col_at_or_after(&line, target);
        self.set_cursor_linewise(
            mode,
            size,
            buf,
            point.with_byte_col(byte_col),
            SetCursorFlags::empty(),
        );
    }

    #[inline]
    pub fn offset(&self) -> Offset {
        self.offset
    }

    pub(crate) fn new(id: ViewId) -> Self {
        Self {
            id,
            number_width: Cell::new(0),
            settings: Default::default(),
            cursor: Default::default(),
            offset: Default::default(),
            jumps: Default::default(),
        }
    }

    /// Split a view from another view: a copy with a new id.
    /// The group is a column of the view's row, so the split isn't in the group.
    pub(crate) fn split_from(id: ViewId, view: View) -> Self {
        assert_ne!(id, view.id);
        Self { id, ..view }
    }

    pub fn jump_list(&self) -> &JumpList<Location> {
        &self.jumps
    }

    pub fn jump_list_mut(&mut self) -> &mut JumpList<Location> {
        &mut self.jumps
    }
}

impl View {}
