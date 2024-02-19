use ropey::Rope;

use crate::{Buffer, BufferId, Editor, Position};

slotmap::new_key_type! {
    pub struct ViewId;
}

pub struct View {
    id: ViewId,
    buf: BufferId,
    cursor: Position,
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
        self.cursor
    }

    #[inline]
    pub fn set_cursor(&mut self, buf: &Buffer, pos: Position) {
        assert_eq!(buf.id(), self.buf);
        let text = buf.text();

        let _: Option<()> = try {
            // check line is in-bounds
            let line = text.get_line(pos.line().idx())?;
            // check column is in-bounds for the line
            if line.get_char(pos.col().idx())? != '\n' {
                self.cursor = pos;
            }
        };
    }

    pub(crate) fn new(id: ViewId, buf: BufferId) -> Self {
        Self { id, buf, cursor: Position::default() }
    }
}
