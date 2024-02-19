use crate::{Buffer, BufferId, Col, Position};

slotmap::new_key_type! {
    pub struct ViewId;
}

pub struct View {
    id: ViewId,
    buf: BufferId,
    cursor: Cursor,
}

#[derive(Clone, Copy, Default)]
struct Cursor {
    pos: Position,
    // remember
    col: Col,
}

impl Cursor {
    fn new(pos: Position, col: Col) -> Self {
        Self { pos, col }
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
    pub fn cursor(&self) -> Position {
        self.cursor.pos
    }

    pub(crate) fn move_cursor_down(&mut self, buf: &Buffer, n: u16) {
        todo!()
    }

    #[inline]
    pub(crate) fn set_cursor(&mut self, buf: &Buffer, pos: Position) {
        assert_eq!(buf.id(), self.buf);
        let text = buf.text();

        // check line is in-bounds
        let line = match text.get_line(pos.line().idx()) {
            Some(line) => line,
            None => return,
        };

        // check column is in-bounds for the line
        self.cursor.pos = match line.get_char(pos.col().idx()) {
            // Cursor is in-bounds for the line
            Some(char) if char != '\n' => pos,
            // Cursor is out of bounds for the line, but the line exists.
            // We move the cursor to the line to the rightmost character.
            _ => pos.with_col(line.len_chars().max(2) - 2),
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
