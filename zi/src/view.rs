use ropey::Rope;

use crate::{BufferId, Editor, Position};

slotmap::new_key_type! {
    pub struct ViewId;
}

pub struct View {
    id: ViewId,
    buffer: BufferId,
    cursor: Position,
}

impl View {
    #[inline]
    pub fn id(&self) -> ViewId {
        self.id
    }

    #[inline]
    pub fn buffer(&self) -> BufferId {
        self.buffer
    }

    #[inline]
    pub fn cursor(&self) -> Position {
        self.cursor
    }

    #[inline]
    pub fn set_cursor(&mut self, _text: &Rope, cursor: Position) {
        // validate etc
        self.cursor = cursor;
    }

    pub(crate) fn new(id: ViewId, buffer: BufferId) -> Self {
        Self { id, buffer, cursor: Position::default() }
    }
}
