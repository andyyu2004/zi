use unicode_width::UnicodeWidthChar;

use crate::editor::cursor::SetCursorFlags;
use crate::{Buffer, BufferId, Col, Direction, Mode, Position};

slotmap::new_key_type! {
    pub struct ViewId;
}

pub struct View {
    id: ViewId,
    buf: BufferId,
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

    /// Returns the cursor coordinates in the buffer in cells (not characters).
    /// For example, '\t' is one character but is 4 cells wide (by default).
    #[inline]
    pub fn cursor_cells(&self, buf: &Buffer) -> (u32, u32) {
        assert_eq!(buf.id(), self.buf);
        let line_idx = self.cursor.pos.line().idx();
        let line = buf.text().line(line_idx);
        let byte = line
            .chars()
            .take(self.cursor.pos.col().idx())
            .map(|c| {
                c.width().unwrap_or_else(|| match c {
                    '\t' => buf.tab_width() as usize,
                    c => panic!("unexpected control character: {c}"),
                })
            })
            .sum::<usize>();
        (byte as u32, line_idx as u32)
    }

    pub(crate) fn move_cursor(&mut self, mode: Mode, buf: &Buffer, direction: Direction) {
        assert_eq!(buf.id(), self.buf);

        let pos = match direction {
            Direction::Left => self.cursor.pos.left(1),
            Direction::Right => self.cursor.pos.right(1),
            // Horizontal movements set the target column.
            // Vertical movements try to keep moving to the target column.
            Direction::Up => self.cursor.pos.up(1).with_col(self.cursor.target_col),
            Direction::Down => self.cursor.pos.down(1).with_col(self.cursor.target_col),
        };

        let flags = if direction.is_vertical() {
            SetCursorFlags::NO_COLUMN_BOUNDS_CHECK
        } else {
            SetCursorFlags::empty()
        };

        self.set_cursor(mode, buf, pos, flags);
    }

    #[inline]
    pub(crate) fn set_cursor(
        &mut self,
        mode: Mode,
        buf: &Buffer,
        pos: Position,
        flags: SetCursorFlags,
    ) {
        assert_eq!(buf.id(), self.buf);
        let text = buf.text();

        // Check line is in-bounds
        let line = match text.get_line(pos.line().idx()) {
            Some(line) => line,
            None => return,
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
        }
    }

    pub(crate) fn new(id: ViewId, buf: BufferId) -> Self {
        Self { id, buf, cursor: Cursor::default() }
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
