use tui::{LineNumber, Rect, Widget as _};
use unicode_width::UnicodeWidthChar;

use crate::editor::cursor::SetCursorFlags;
use crate::editor::Resource;
use crate::position::{Offset, RangeMergeIter, Size};
use crate::{
    buffer, Buffer, BufferId, Col, Direction, Editor, JumpList, LazyText, Location, Mode, Point,
    Url,
};

slotmap::new_key_type! {
    pub struct ViewId;
    pub struct ViewGroupId;
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
    line_number: LineNumber,
    group: Option<ViewGroupId>,
    url: Url,
    jumps: JumpList<Location>,
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
    pos: Point,
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
        Self { pos, target_col: pos.col() }
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
    pub fn with_line_number(self, line_number: LineNumber) -> Self {
        Self { line_number, ..self }
    }

    #[inline]
    pub fn group(&self) -> Option<ViewGroupId> {
        self.group
    }

    #[inline]
    pub fn set_group(&mut self, group: ViewGroupId) {
        self.group = Some(group);
    }

    #[inline]
    pub fn with_group(self, group: ViewGroupId) -> Self {
        Self { group: Some(group), ..self }
    }

    #[inline]
    pub fn set_buffer(&mut self, buf: BufferId) {
        self.buf = buf;
        self.cursor = Cursor::default();
        self.offset = Offset::default();
    }

    #[inline]
    pub fn cursor(&self) -> Point {
        self.cursor.pos
    }

    /// Returns the cursor coordinates in the buffer in cells (not characters) relative to the viewport.
    /// For example, '\t' is one character but is 4 cells wide (by default).
    #[inline]
    pub(crate) fn cursor_viewport_coords(&self, buf: &dyn Buffer) -> (u16, u16) {
        assert_eq!(buf.id(), self.buf);
        assert!(
            self.offset.line <= self.cursor.pos.line().idx() as u32,
            "cursor is above the viewport: offset={} cursor={}",
            self.offset,
            self.cursor.pos,
        );
        assert!(
            self.offset.col <= self.cursor.pos.col().idx() as u32,
            "cursor is to the left of the viewport"
        );

        let line_idx = self.cursor.pos.line().idx();
        let line = buf.text().line(line_idx);
        let byte = line
            .chars()
            .take(self.cursor.pos.col().idx())
            .map(|c| {
                c.width().unwrap_or_else(|| match c {
                    '\t' => buf.tab_width() as usize,
                    _ => 0,
                })
            })
            .sum::<usize>();
        // TODO need tests for the column adjustment
        let x = byte as u32 - self.offset.col;
        let y = line_idx as u32 - self.offset.line;
        (x.try_into().unwrap(), y.try_into().unwrap())
    }

    pub(crate) fn move_cursor(
        &mut self,
        mode: Mode,
        size: impl Into<Size>,
        buf: &dyn Buffer,
        direction: Direction,
        amt: u32,
    ) -> Point {
        assert_eq!(buf.id(), self.buf);

        let pos = match direction {
            Direction::Left => self.cursor.pos.left(amt),
            Direction::Right => self.cursor.pos.right(amt),
            // Horizontal movements set the target column.
            // Vertical movements try to keep moving to the target column.
            Direction::Up => self.cursor.pos.up(amt).with_col(self.cursor.target_col),
            Direction::Down => self.cursor.pos.down(amt).with_col(self.cursor.target_col),
        };

        let mut flags = if direction.is_vertical() {
            SetCursorFlags::NO_COLUMN_BOUNDS_CHECK
        } else {
            SetCursorFlags::empty()
        };

        // If we're moving down and we overshoot, move to the last line instead of doing nothing.
        flags |= SetCursorFlags::MOVE_TO_LAST_LINE_IF_OUT_OF_BOUNDS;

        self.set_cursor(mode, size, buf, pos, flags)
    }

    #[inline]
    pub(crate) fn set_cursor(
        &mut self,
        mode: Mode,
        size: impl Into<Size>,
        buf: &dyn Buffer,
        pos: Point,
        flags: SetCursorFlags,
    ) -> Point {
        assert_eq!(buf.id(), self.buf);
        let text = buf.text();
        let size = size.into();

        // Check line is in-bounds
        let mut line_idx = pos.line().idx();
        let line = match text.get_line(line_idx) {
            // Disallow putting cursor on the final empty line.
            // Note we're using `line_in_bounds` instead of `line_idx < text.len_lines() - 1`
            // `line_in_bounds` is `O(line_idx)` and `len_lines` can be `O(n)`.
            Some(line) if line != "" || text.line_in_bounds(line_idx + 2) => line,
            _ if flags.contains(SetCursorFlags::MOVE_TO_LAST_LINE_IF_OUT_OF_BOUNDS) => {
                line_idx = text.len_lines().saturating_sub(2);
                text.line(line_idx)
            }
            _ => return self.cursor.pos,
        };

        let line_len = line.chars().count();

        let pos = Point::new(line_idx, pos.col());

        // Pretending CRLF doesn't exist.
        // We don't allow the cursor on the newline character.
        let n: usize = match line.get_char(line_len.saturating_sub(1)) {
            Some('\n') => 1,
            _ => 0,
        };

        // Normal mode not allowed to move past the end of the line.
        let n = match mode {
            Mode::Insert => n,
            Mode::Normal | Mode::Command | Mode::Visual => n + 1,
        };

        let max_col = Col::from(line_len.saturating_sub(n));

        // Store where we really want to be without the following bounds constraints.
        self.cursor.target_col = pos.col();
        if !flags.contains(SetCursorFlags::NO_COLUMN_BOUNDS_CHECK) {
            // By default, we want to ensure the target column is in-bounds for the line.
            self.cursor.target_col = self.cursor.target_col.min(max_col);
        }

        // check column is in-bounds for the line
        self.cursor.pos = match line.get_char(pos.col().idx()) {
            // Cursor is in-bounds for the line
            Some(char) if char != '\n' => pos,
            // Cursor is out of bounds for the line, but the line exists.
            // We move the cursor to the line to the rightmost character.
            _ => pos.with_col(max_col),
        };

        // Scroll the view if the cursor moves out of bounds
        if self.cursor.pos.line().raw() < self.offset.line {
            self.offset.line = self.cursor.pos.line().idx() as u32;
        } else if self.cursor.pos.line().raw() >= self.offset.line + size.height as u32 {
            self.offset.line = self.cursor.pos.line().idx() as u32 - size.height as u32 + 1;
        }

        self.cursor.pos
    }

    pub(crate) fn scroll(
        &mut self,
        mode: Mode,
        size: impl Into<Size>,
        buf: &dyn Buffer,
        direction: Direction,
        amt: u32,
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
            self.cursor.pos.line().raw() >= self.offset.line
                && self.cursor.pos.line().raw() < self.offset.line + size.height as u32,
            "cursor is out of bounds: cursor={} offset={} size={}",
            self.cursor.pos,
            self.offset,
            size
        );
    }

    pub fn line_number_width(&self) -> u8 {
        self.line_number.width()
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
            group: Default::default(),
            cursor: Default::default(),
            offset: Default::default(),
            line_number: Default::default(),
            jumps: JumpList::default(),
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

pub trait HasViewId {
    fn view_id(&self) -> ViewId;
}

impl<V: HasViewId> HasViewId for &V {
    #[inline]
    fn view_id(&self) -> ViewId {
        (*self).view_id()
    }
}

impl HasViewId for ViewId {
    #[inline]
    fn view_id(&self) -> ViewId {
        *self
    }
}

impl HasViewId for View {
    #[inline]
    fn view_id(&self) -> ViewId {
        self.id
    }
}

impl View {
    #[tracing::instrument(skip_all)]
    pub(crate) fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        assert_eq!(surface.area.intersection(area), area);

        let buf = editor.buffer(self.buf);
        let mut query_cursor = tree_sitter::QueryCursor::new();
        query_cursor.set_match_limit(256);
        let theme = editor.theme();

        let line = self.offset().line as usize;

        // FIXME compute highlights only for the necessary range
        let syntax_highlights = buf
            .syntax_highlights(&mut query_cursor)
            .skip_while(|(range, _)| range.end().line().idx() < line)
            .filter_map(|(range, id)| Some((range, id.style(theme)?)))
            .map(|(range, style)| (range - Offset::new(line as u32, 0), style));

        let overlay_highlights = buf
            .overlay_highlights(self, area.into())
            .skip_while(|(range, _)| range.end().line().idx() < line)
            .filter_map(|(range, id)| Some((range, id.style(theme)?)))
            .map(|(range, style)| (range - Offset::new(line as u32, 0), style));

        let highlights =
            RangeMergeIter::new(syntax_highlights, overlay_highlights).inspect(|(range, style)| {
                tracing::trace!(%range, %style, "highlight");
            });

        let chunks = buffer::annotate(buf.text().lines_at(line), highlights);

        let lines = tui::Lines::new(
            line,
            self.line_number,
            buf.tab_width(),
            chunks
                .inspect(|(_, text, _)| tracing::trace!(?text, "render chunk"))
                .map(|(line, text, style)| (line.idx(), text, style.unwrap_or_default().into())),
        );

        surface.set_style(area, tui::Style::default().bg(tui::Color::Rgb(0x00, 0x2b, 0x36)));
        lines.render(area, surface);
    }
}
