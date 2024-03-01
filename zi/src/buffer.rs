mod text;

use std::path::{Path, PathBuf};

use ropey::Rope;
use tree_sitter::{QueryCursor, Range};
use zi_lsp::lsp_types::Url;

pub use self::text::TextBuffer;
use crate::syntax::{HighlightId, HighlightMap, Highlights, Syntax, Theme};
use crate::{FileType, Position};

slotmap::new_key_type! {
    pub struct BufferId;
}

pub trait Buffer {
    fn id(&self) -> BufferId;
    fn path(&self) -> &Path;
    fn url(&self) -> Option<Url>;
    fn language_id(&self) -> &FileType;
    fn tab_width(&self) -> u8;
    fn text(&self) -> &Rope;
    fn version(&self) -> u32;

    // TODO this should be a more general mutate operation
    fn insert_char(&mut self, pos: Position, c: char);

    fn highlights<'a>(
        &'a self,
        cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + 'a>;
}

impl Buffer for Box<dyn Buffer> {
    fn id(&self) -> BufferId {
        self.as_ref().id()
    }

    fn path(&self) -> &Path {
        self.as_ref().path()
    }

    fn url(&self) -> Option<Url> {
        self.as_ref().url()
    }

    fn language_id(&self) -> &FileType {
        self.as_ref().language_id()
    }

    fn tab_width(&self) -> u8 {
        self.as_ref().tab_width()
    }

    fn text(&self) -> &Rope {
        self.as_ref().text()
    }

    fn version(&self) -> u32 {
        self.as_ref().version()
    }

    fn insert_char(&mut self, pos: Position, c: char) {
        self.as_mut().insert_char(pos, c);
    }

    fn highlights<'a>(
        &'a self,
        cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + 'a> {
        self.as_ref().highlights(cursor)
    }
}
