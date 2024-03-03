mod picker;
mod text;

use std::ops::Bound;
use std::path::{Path, PathBuf};

use ropey::{Rope, RopeSlice};
use tree_sitter::{Node, QueryCursor};
use zi_lsp::lsp_types::Url;

pub use self::picker::PickerBuffer;
pub use self::text::TextBuffer;
use crate::syntax::{HighlightId, HighlightMap, Highlights, Syntax, Theme};
use crate::{FileType, Position, Range};

slotmap::new_key_type! {
    pub struct BufferId;
}

pub trait Buffer {
    fn id(&self) -> BufferId;
    fn path(&self) -> &Path;
    fn url(&self) -> Option<Url>;
    fn language_id(&self) -> &FileType;
    fn tab_width(&self) -> u8;
    fn text(&self) -> RopeSlice<'_>;
    fn version(&self) -> u32;

    // TODO this should be a more general mutate operation
    fn insert_char(&mut self, pos: Position, c: char);

    fn syntax_highlights<'a>(
        &'a self,
        _cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = (Node<'_>, HighlightId)> + 'a> {
        Box::new(std::iter::empty())
    }

    fn overlay_highlights(&self) -> Box<dyn Iterator<Item = (Range, HighlightId)> + '_> {
        Box::new(std::iter::empty())
    }

    fn writable_text(&self) -> RopeSlice<'_> {
        self.text().slice(self.writable_range())
    }

    /// The range of the buffer that can be written to in character indices
    fn writable_range(&self) -> (Bound<usize>, Bound<usize>) {
        (Bound::Unbounded, Bound::Unbounded)
    }

    fn boxed(self) -> Box<dyn Buffer>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }

    /// Called just before rendering the buffer
    fn pre_render(&mut self) {}

    /// Called when a view is closed that was displaying this buffer
    fn on_leave(&mut self) {}
}

// NOTE: remember to add all the methods to the Box<dyn Buffer> impl below, including default methods
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

    fn text(&self) -> RopeSlice<'_> {
        self.as_ref().text().slice(..)
    }

    fn version(&self) -> u32 {
        self.as_ref().version()
    }

    fn insert_char(&mut self, pos: Position, c: char) {
        self.as_mut().insert_char(pos, c);
    }

    fn syntax_highlights<'a>(
        &'a self,
        cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = (Node<'a>, HighlightId)> + 'a> {
        self.as_ref().syntax_highlights(cursor)
    }

    fn overlay_highlights(&self) -> Box<dyn Iterator<Item = (Range, HighlightId)> + '_> {
        self.as_ref().overlay_highlights()
    }

    fn writable_range(&self) -> (Bound<usize>, Bound<usize>) {
        self.as_ref().writable_range()
    }

    fn boxed(self) -> Box<dyn Buffer>
    where
        Self: Sized + 'static,
    {
        self
    }

    fn pre_render(&mut self) {
        self.as_mut().pre_render();
    }

    fn on_leave(&mut self) {
        self.as_mut().on_leave();
    }
}
