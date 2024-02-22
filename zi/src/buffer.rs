use ropey::Rope;
use tree_sitter::{QueryCapture, QueryCursor};

use crate::syntax::{Highlights, Syntax};
use crate::Position;

slotmap::new_key_type! {
    pub struct BufferId;
}

pub struct Buffer {
    id: BufferId,
    text: Rope,
    syntax: Option<Syntax>,
}

impl Buffer {
    #[inline]
    pub fn new(id: BufferId, text: Rope) -> Self {
        // FIXME, detect language somewhere
        let mut syntax = Syntax::rust();
        syntax.apply(text.slice(..));
        Self { id, text, syntax: Some(syntax) }
    }

    #[inline]
    pub fn id(&self) -> BufferId {
        self.id
    }

    #[inline]
    pub fn text(&self) -> &Rope {
        &self.text
    }

    #[inline]
    pub(crate) fn insert_char(&mut self, pos: Position, c: char) {
        let idx = self.text.line_to_char(pos.line().idx()) + pos.col().idx();
        self.text.insert_char(idx, c);
        if let Some(syntax) = self.syntax.as_mut() {
            syntax.apply(self.text.slice(..));
        }
    }

    pub fn highlights<'a>(
        &'a self,
        cursor: &'a mut QueryCursor,
    ) -> impl IntoIterator<Item = QueryCapture<'a>> {
        self.syntax
            .as_ref()
            .map_or(Highlights::Empty, |syntax| syntax.highlights(cursor, self.text.slice(..)))
    }
}
