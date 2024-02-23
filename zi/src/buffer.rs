use ropey::Rope;
use tree_sitter::{Node, QueryCursor};

use crate::syntax::{HighlightId, HighlightMap, Highlights, Syntax, Theme};
use crate::{LanguageServerId, Position};

slotmap::new_key_type! {
    pub struct BufferId;
}

pub struct Buffer {
    id: BufferId,
    text: Rope,
    syntax: Option<Syntax>,
    // FIXME highlight map doesn't belong here
    highlight_map: HighlightMap,
    language_servers: Vec<LanguageServerId>,
}

impl Buffer {
    #[inline]
    pub fn new(id: BufferId, text: Rope, theme: &Theme) -> Self {
        // FIXME, detect language somewhere
        let mut syntax = Syntax::rust();
        syntax.apply(text.slice(..));
        Self {
            id,
            text,
            highlight_map: HighlightMap::new(syntax.highlights_query().capture_names(), theme),
            syntax: Some(syntax),
            language_servers: Default::default(),
        }
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
    ) -> impl Iterator<Item = (Node<'a>, HighlightId)> + 'a {
        self.syntax
            .as_ref()
            .map_or(Highlights::Empty, |syntax| syntax.highlights(cursor, self.text.slice(..)))
            .map(|capture| (capture.node, self.highlight_map.get(capture.index)))
    }
}
