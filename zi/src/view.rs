use unicode_width::UnicodeWidthChar;

use crate::editor::cursor::SetCursorFlags;
use crate::position::{Offset, Size};
use crate::{Buffer, BufferId, Col, Direction, Mode, Position};

slotmap::new_key_type! {
    pub struct ViewId;
}

pub struct View {
    id: ViewId,
    /// The buffer that this view is displaying.
    buf: BufferId,
    /// The offset of the view in the buffer.
    /// i.e. this changes on scroll.
    offset: Offset,
    /// The cursor position in the buffer (relative to the offset).
    cursor: Cursor,
}

#[derive(Clone, Copy, Default)]
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

    pub fn set_buffer(&mut self, buf: BufferId) {
        self.buf = buf;
    }

    #[inline]
    pub fn cursor(&self) -> Position {
        self.cursor.pos
    }

    /// Returns the cursor coordinates in the buffer in cells (not characters) relative to the viewport.
    /// For example, '\t' is one character but is 4 cells wide (by default).
    #[inline]
    pub fn cursor_viewport_coords(&self, buf: &Buffer) -> (u32, u32) {
        assert_eq!(buf.id(), self.buf);
        assert!(
            self.offset.line <= self.cursor.pos.line().idx() as u32,
            "cursor is above the viewport"
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
        (byte as u32 - self.offset.col, line_idx as u32 - self.offset.line)
    }

    pub(crate) fn move_cursor(
        &mut self,
        mode: Mode,
        size: Size,
        buf: &Buffer,
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

        let flags = if direction.is_vertical() {
            SetCursorFlags::NO_COLUMN_BOUNDS_CHECK
        } else {
            SetCursorFlags::empty()
        };

        self.set_cursor(mode, size, buf, pos, flags);
    }

    #[inline]
    pub(crate) fn set_cursor(
        &mut self,
        mode: Mode,
        size: Size,
        buf: &Buffer,
        pos: Position,
        flags: SetCursorFlags,
    ) {
        assert_eq!(buf.id(), self.buf);
        let text = buf.text();

        // Check line is in-bounds
        let line_idx = pos.line().idx();
        let line = match text.get_line(line_idx) {
            // disallow putting cursor on the final empty line
            Some(line) if line != "" || line_idx < text.len_lines() - 1 => line,
            _ => return,
        };

        // Pretending CRLF doesn't exist.
        // We don't allow the cursor on the newline character.
        let n: usize = match line.get_char(line.len_chars().saturating_sub(1)) {
            Some('\n') => 1,
            _ => 0,
        };

        // Normal mode not allowed to move past the end of the line.
        let n = match mode {
            Mode::Normal => n + 1,
            Mode::Insert => n,
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
        size: Size,
        buf: &Buffer,
        direction: Direction,
        amt: u32,
    ) {
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
    }

    #[inline]
    pub fn offset(&self) -> Offset {
        self.offset
    }

    pub(crate) fn new(id: ViewId, buf: BufferId) -> Self {
        Self { id, buf, cursor: Cursor::default(), offset: Default::default() }
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
