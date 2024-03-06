use tui::{Clear, Rect, Widget as _};
use unicode_width::UnicodeWidthChar;

use crate::editor::cursor::SetCursorFlags;
use crate::position::{Offset, RangeMergeIter, Size};
use crate::{Buffer, BufferId, Col, Direction, Editor, Mode, Position, Range};

slotmap::new_key_type! {
    pub struct ViewId;
}

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
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Cursor {
    pos: Position,
    // When we move the cursor down we may go to a shorter line, virtual column stores the column
    // that the cursor should really be at, but can't be because the line is too short.
    target_col: Col,
}

impl From<Position> for Cursor {
    fn from(pos: Position) -> Self {
        Self::new(pos)
    }
}

impl Cursor {
    fn new(pos: Position) -> Self {
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
    pub fn set_buffer(&mut self, buf: BufferId) {
        self.buf = buf;
        self.cursor = Cursor::default();
        self.offset = Offset::default();
    }

    #[inline]
    pub fn cursor(&self) -> Position {
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
    ) {
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

        self.set_cursor(mode, size, buf, pos, flags);
    }

    #[inline]
    pub(crate) fn set_cursor(
        &mut self,
        mode: Mode,
        size: impl Into<Size>,
        buf: &dyn Buffer,
        pos: Position,
        flags: SetCursorFlags,
    ) {
        assert_eq!(buf.id(), self.buf);
        let text = buf.writable_text();
        let size = size.into();

        // Check line is in-bounds
        let mut line_idx = pos.line().idx();
        let line = match text.get_line(line_idx) {
            // disallow putting cursor on the final empty line
            Some(line) if line != "" || line_idx < text.len_lines() - 1 => line,
            _ if flags.contains(SetCursorFlags::MOVE_TO_LAST_LINE_IF_OUT_OF_BOUNDS) => {
                line_idx = text.len_lines().saturating_sub(2);
                text.line(line_idx)
            }
            _ => return,
        };

        let pos = Position::new(line_idx, pos.col());

        // Pretending CRLF doesn't exist.
        // We don't allow the cursor on the newline character.
        let n: usize = match line.get_char(line.len_chars().saturating_sub(1)) {
            Some('\n') => 1,
            _ => 0,
        };

        // Normal mode not allowed to move past the end of the line.
        let n = match mode {
            Mode::Insert => n,
            Mode::Normal | Mode::Visual => n + 1,
        };

        let max_col = Col::from(line.len_chars().saturating_sub(n));

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
        match direction {
            Direction::Up => self.offset.line = self.offset.line.saturating_sub(amt),
            Direction::Down => {
                self.offset.line = self
                    .offset
                    .line
                    .saturating_add(amt)
                    .min(buf.text().len_lines().saturating_sub(2) as u32)
            }
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

    #[inline]
    pub fn offset(&self) -> Offset {
        self.offset
    }

    pub(crate) fn new(id: ViewId, buf: BufferId) -> Self {
        Self { id, buf, cursor: Cursor::default(), offset: Default::default() }
    }

    pub(crate) fn new_from(id: ViewId, view: View) -> Self {
        assert_ne!(id, view.id);
        Self { id, ..view }
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
            .skip_while(|(node, _)| node.range().end_point.row < line)
            .filter_map(|(node, id)| Some((node, id.style(theme)?)))
            .map(|(node, style)| {
                let range = Range::from(node.range());
                // Need to adjust the line to be 0-based as that's what `tui::Lines` is assuming
                (range - Offset::new(line as u32, 0), style)
            });

        let overlay_highlights = buf
            .overlay_highlights(self, area.into())
            .skip_while(|(range, _)| range.end.line().idx() < line)
            .filter_map(|(range, id)| Some((range, id.style(theme)?)))
            .map(|(range, style)| (range - Offset::new(line as u32, 0), style));

        let highlights = RangeMergeIter::new(syntax_highlights, overlay_highlights)
            .map(|(range, style)| (range.into(), style.into()));

        const LINE_NR_WIDTH: usize = 4;
        let lines = tui::Lines::new(
            line,
            LINE_NR_WIDTH,
            buf.tab_width(),
            buf.text().lines_at(line),
            highlights,
        );

        Clear.render(area, surface);
        surface.set_style(area, tui::Style::default().bg(tui::Color::Rgb(0x00, 0x2b, 0x36)));
        lines.render(area, surface);
    }
}
