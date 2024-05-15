use std::cell::Cell;

use tui::LineNumberStyle;
use zi_core::{Offset, Size};
use zi_text::{self, Text as _, TextSlice};

use crate::buffer::Buffer;
use crate::config::Setting;
use crate::editor::{Resource, Selector};
use crate::private::Sealed;
use crate::{BufferId, Col, Direction, Editor, JumpList, Location, Mode, Point, Url};

/// View-local configuration
#[derive(Clone, Debug)]
pub struct Config {
    /// The width of the line numbers column including a space between the number and the text
    pub line_number_width: Setting<u8>,
    pub line_number_style: Setting<LineNumberStyle>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            line_number_width: Setting::new(4),
            line_number_style: Setting::new(LineNumberStyle::Absolute),
        }
    }
}

slotmap::new_key_type! {
    pub struct ViewId;
    pub struct ViewGroupId;
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SetCursorFlags: u8 {
        /// Shift the cursor right to the first non-whitespace character if necessary.
        const START_OF_LINE = 1 << 0;
        /// Do not update the cursor target column even if the cursor did not move.
        const NO_FORCE_UPDATE_TARGET = 1 << 1;
        /// Use the target column as the cursor column. Implies `NO_FORCE_UPDATE_TARGET`.
        const USE_TARGET_COLUMN = 1 << 2 | Self::NO_FORCE_UPDATE_TARGET.bits();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}

impl Sealed for ViewId {}

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
#[derive(Debug, Clone)]
pub struct View {
    id: ViewId,
    /// The buffer that this view is displaying.
    buf: BufferId,
    /// The offset of the view in the buffer.
    /// i.e. this changes on scroll.
    offset: Offset,
    /// The cursor position in the buffer
    cursor: Cursor,
    group: Option<ViewGroupId>,
    url: Url,
    jumps: JumpList<Location>,
    config: Config,

    /// The actual width of the line numbers column including a space between the number and the text.
    /// This should be at least `config.line_number_width` but can be larger if the line numbers are wider.
    /// This value should be updated when rendering the view.
    pub(crate) number_width: Cell<u16>,
}

impl Sealed for View {}

impl Selector<ViewId> for View {
    #[inline]
    fn select(&self, _editor: &Editor) -> ViewId {
        self.id()
    }
}

impl Selector<BufferId> for View {
    #[inline]
    fn select(&self, _editor: &Editor) -> BufferId {
        self.buffer()
    }
}

impl Resource for View {
    type Id = ViewId;

    const URL_SCHEME: &'static str = "view";

    fn id(&self) -> Self::Id {
        self.id
    }

    fn url(&self) -> &Url {
        assert_eq!(self.url.scheme(), Self::URL_SCHEME);
        &self.url
    }
}

/// A view group is a collection of views that are displayed together,
/// closing one view in the group closes all views in the group.
/// A view can be in at most one group.
#[derive(Debug, Clone)]
pub struct ViewGroup {
    id: ViewGroupId,
    url: Url,
}

impl Resource for ViewGroup {
    type Id = ViewGroupId;

    const URL_SCHEME: &'static str = "view-group";

    fn id(&self) -> Self::Id {
        self.id
    }

    fn url(&self) -> &Url {
        assert_eq!(self.url.scheme(), Self::URL_SCHEME);
        &self.url
    }
}

impl ViewGroup {
    pub fn new(id: ViewGroupId, url: Url) -> Self {
        Self { id, url }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Cursor {
    point: Point,
    // When we move the cursor down we may go to a shorter line, virtual column stores the column
    // that the cursor should really be at, but can't be because the line is too short.
    target_col: Col,
}

impl From<Point> for Cursor {
    fn from(pos: Point) -> Self {
        Self::new(pos)
    }
}

impl Cursor {
    fn new(pos: Point) -> Self {
        Self { point: pos, target_col: pos.col() }
    }
}

impl View {
    #[inline]
    pub fn id(&self) -> ViewId {
        self.id
    }

    #[inline]
    pub fn buffer(&self) -> BufferId {
        self.buf
    }

    #[inline]
    pub fn config(&self) -> &Config {
        &self.config
    }

    #[inline]
    pub fn group(&self) -> Option<ViewGroupId> {
        self.group
    }

    #[inline]
    pub fn cursor(&self) -> Point {
        self.cursor.point
    }

    pub(crate) fn cursor_target_col(&self) -> Col {
        self.cursor.target_col
    }

    #[inline]
    pub(crate) fn set_group(&mut self, group: ViewGroupId) {
        self.group = Some(group);
    }

    #[inline]
    pub(crate) fn with_group(self, group: ViewGroupId) -> Self {
        Self { group: Some(group), ..self }
    }

    #[inline]
    pub(crate) fn set_buffer(&mut self, buf: BufferId) {
        self.buf = buf;
        self.cursor = Cursor::default();
        self.offset = Offset::default();
    }

    pub(crate) fn align(
        &mut self,
        size: impl Into<Size>,
        buf: &dyn Buffer,
        alignment: VerticalAlignment,
    ) {
        assert_eq!(buf.id(), self.buf);
        let cursor = self.cursor();
        let size = size.into();

        let line = match alignment {
            VerticalAlignment::Top => cursor.line(),
            VerticalAlignment::Center => cursor.line().saturating_sub(size.height as usize / 2),
            VerticalAlignment::Bottom => {
                cursor.line().saturating_sub(size.height.saturating_sub(1) as usize)
            }
        };

        self.offset = Offset::new(line, 0);
    }

    /// Returns the cursor coordinates in the buffer in cells (not characters) relative to the viewport.
    /// For example, '\t' is one character but is 4 cells wide (by default).
    #[inline]
    pub(crate) fn cursor_viewport_coords(&self, buf: &dyn Buffer) -> (u16, u16) {
        assert_eq!(buf.id(), self.buf);
        assert!(
            self.offset.line <= self.cursor.point.line(),
            "cursor is above the viewport: offset={} cursor={}",
            self.offset,
            self.cursor.point,
        );
        assert!(
            self.offset.col <= self.cursor.point.col(),
            "cursor is to the left of the viewport"
        );

        let line_idx = self.cursor.point.line();
        let text = buf.text();
        let line = text.get_line(line_idx).unwrap_or_else(|| Box::new(""));
        let byte =
            line.chars().take(self.cursor.point.col()).map(|c| buf.char_width(c)).sum::<usize>();
        // TODO need tests for the column adjustment
        let x = byte - self.offset.col;
        let y = line_idx - self.offset.line;
        (x.try_into().unwrap(), y.try_into().unwrap())
    }

    /// `amt` is measured in characters or lines depending on the direction.
    pub(crate) fn move_cursor(
        &mut self,
        mode: Mode,
        size: impl Into<Size>,
        buf: &dyn Buffer,
        direction: Direction,
        amt: usize,
    ) -> Point {
        assert_eq!(buf.id(), self.buf);

        let pos = match direction {
            Direction::Left => match buf.text().char_before_point(self.cursor.point) {
                // this is wrong, can't assume all characters are the same width
                Some(c) => self.cursor.point.left(c.len_utf8() * amt),
                None => return self.cursor.point,
            },
            Direction::Right => match buf.text().char_at_point(self.cursor.point) {
                Some(c) => self.cursor.point.right(c.len_utf8() * amt),
                None => return self.cursor.point,
            },
            // Horizontal movements set the target column.
            // Vertical movements try to keep moving to the target column.
            Direction::Up => self.cursor.point.up(amt),
            Direction::Down => self.cursor.point.down(amt),
        };

        let flags = match direction {
            Direction::Up | Direction::Down => SetCursorFlags::USE_TARGET_COLUMN,
            _ => SetCursorFlags::empty(),
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
        buf: &dyn Buffer,
        mut byte: usize,
        flags: SetCursorFlags,
    ) -> Point {
        assert_eq!(buf.id(), self.buf);

        assert!(
            !flags.contains(SetCursorFlags::USE_TARGET_COLUMN),
            "not applicable for bytewise cursor movement"
        );

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
                byte -= chars.next().map_or(0, |c| c.len_utf8());
            }
            chars
        } else {
            text.byte_slice(..byte).chars().rev().peekable()
        };

        // Prevent cursor being past end of line
        if !insert && text.char_at_byte(byte) == Some('\n') && chars.peek() != Some(&'\n') {
            byte = byte.saturating_sub('\n'.len_utf8());
        }

        let pos = text.byte_to_point(byte);

        if self.cursor.point != pos && self.cursor.target_col != pos.col() {
            self.cursor = Cursor::new(pos);
        } else {
            self.cursor.point = pos;
            if !flags.contains(SetCursorFlags::NO_FORCE_UPDATE_TARGET) {
                self.cursor.target_col = pos.col();
            }
        }

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
        buf: &dyn Buffer,
        mut pos: Point,
        flags: SetCursorFlags,
    ) -> Point {
        assert_eq!(buf.id(), self.buf);
        let text = buf.text();
        let size = size.into();

        if flags.contains(SetCursorFlags::USE_TARGET_COLUMN) {
            pos = pos.with_col(self.cursor.target_col);
        }

        // Check line is in-bounds
        let mut line_idx = pos.line();
        let line = match text.get_line(line_idx) {
            Some(line) => line,
            _ => {
                line_idx = text.len_lines().saturating_sub(1);
                text.get_line(line_idx).unwrap_or_else(|| Box::new(""))
            }
        };

        let line_len = line.len_bytes();

        let pos = Point::new(line_idx, pos.col());

        // Normal mode not allowed to move past the end of the line.
        let k = match mode {
            Mode::Insert => 0,
            Mode::Normal | Mode::Command | Mode::Visual | Mode::OperatorPending(..) => {
                line.chars().next_back().map_or(0, |c| c.len_utf8())
            }
        };

        let max_col = Col::from(line_len.saturating_sub(k));

        // check column is in-bounds for the line
        let new_cursor = match pos.col() {
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

                    pos.with_col(col.max(i))
                } else {
                    pos.with_col(max_col.min(i))
                }
            }
            // Cursor is out of bounds for the line, but the line exists.
            // We move the cursor to the line to the rightmost character.
            _ => pos.with_col(max_col),
        };

        if self.cursor.target_col != pos.col() && self.cursor.point != new_cursor
            || !flags.contains(SetCursorFlags::NO_FORCE_UPDATE_TARGET)
        {
            self.cursor.target_col = new_cursor.col();
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
        buf: &dyn Buffer,
        direction: Direction,
        amt: usize,
    ) {
        let size = size.into();
        let prev = self.offset;
        // don't need to bounds check the scroll, `move_cursor` handles that
        match direction {
            Direction::Up => self.offset.line = self.offset.line.saturating_sub(amt),
            Direction::Down => self.offset.line = self.offset.line.saturating_add(amt),
            Direction::Left => self.offset.col = self.offset.col.saturating_sub(amt),
            Direction::Right => self.offset.col = self.offset.col.saturating_add(amt),
        }

        // Move the cursor the same amount to match.
        let amt = match direction {
            Direction::Up => prev.line - self.offset.line,
            Direction::Down => self.offset.line - prev.line,
            Direction::Left => prev.col - self.offset.col,
            Direction::Right => self.offset.col - prev.col,
        };

        self.move_cursor(mode, size, buf, direction, amt);
        assert!(
            self.cursor.point.line() >= self.offset.line
                && self.cursor.point.line() < self.offset.line + size.height as usize,
            "cursor is out of bounds: cursor={} offset={} size={size}",
            self.cursor.point,
            self.offset,
        );
    }

    #[inline]
    pub fn offset(&self) -> Offset {
        self.offset
    }

    pub(crate) fn new(id: ViewId, buf: BufferId) -> Self {
        Self {
            id,
            url: Url::parse(&format!("view://{}", id.0.as_ffi())).unwrap(),
            buf,
            number_width: Cell::new(0),
            config: Default::default(),
            group: Default::default(),
            cursor: Default::default(),
            offset: Default::default(),
            jumps: Default::default(),
        }
    }

    /// Split a view from another view.
    /// Similar to `clone` but only copies a specific set of fields.
    /// In particular, it does not copy the group.
    pub(crate) fn split_from(id: ViewId, view: View) -> Self {
        assert_ne!(id, view.id);
        Self { id, group: None, ..view.clone() }
    }

    pub fn jump_list(&self) -> &JumpList<Location> {
        &self.jumps
    }

    pub fn jump_list_mut(&mut self) -> &mut JumpList<Location> {
        &mut self.jumps
    }
}

impl View {}
