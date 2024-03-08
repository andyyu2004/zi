mod explorer;
mod picker;
mod text;

use std::any::Any;
use std::fmt;
use std::ops::Bound;
use std::path::{Path, PathBuf};

use ropey::{Rope, RopeSlice};
use stdx::sync::Cancel;
use tree_sitter::QueryCursor;
use zi_lsp::lsp_types::Url;

pub use self::explorer::ExplorerBuffer;
pub use self::picker::PickerBuffer;
pub use self::text::TextBuffer;
use crate::keymap::Keymap;
use crate::syntax::{HighlightId, HighlightMap, Highlights, Syntax, Theme};
use crate::{FileType, Position, Range, Size, View};

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
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    // TODO this should be a more general mutate operation
    fn insert_char(&mut self, pos: Position, c: char);

    /// Syntax highlights iterator.
    /// All ranges must be single-line ranges.
    fn syntax_highlights<'a>(
        &'a self,
        _cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + 'a> {
        Box::new(std::iter::empty())
    }

    /// Overlay highlights iterator that are merged with the syntax highlights.
    /// Overlay highlights take precedence.
    /// All ranges must be single-line ranges.
    fn overlay_highlights(
        &self,
        _view: &View,
        _size: Size,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + '_> {
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

    fn keymap(&mut self) -> Option<&mut Keymap> {
        None
    }

    /// Called just before rendering the buffer
    fn pre_render(&mut self, view: &View, _area: tui::Rect) {
        assert_eq!(self.id(), view.buffer());
    }

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
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + 'a> {
        self.as_ref().syntax_highlights(cursor)
    }

    fn overlay_highlights(
        &self,
        view: &View,
        size: Size,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + '_> {
        self.as_ref().overlay_highlights(view, size)
    }

    fn writable_range(&self) -> (Bound<usize>, Bound<usize>) {
        self.as_ref().writable_range()
    }

    fn keymap(&mut self) -> Option<&mut Keymap> {
        self.as_mut().keymap()
    }

    fn boxed(self) -> Box<dyn Buffer>
    where
        Self: Sized + 'static,
    {
        self
    }

    fn pre_render(&mut self, view: &View, area: tui::Rect) {
        self.as_mut().pre_render(view, area);
    }

    fn on_leave(&mut self) {
        self.as_mut().on_leave();
    }

    fn as_any(&self) -> &dyn Any {
        self.as_ref().as_any()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.as_mut().as_any_mut()
    }
}

pub trait Item: fmt::Display + Clone + Sync + Send + 'static {}

impl<T> Item for T where T: fmt::Display + Clone + Sync + Send + 'static {}

/// Wrapper around a `nucleo::Injector` with cancellation support
pub struct Injector<T> {
    injector: nucleo::Injector<T>,
    cancel: Cancel,
}

impl<T: Item> Injector<T> {
    pub fn new(injector: nucleo::Injector<T>, cancel: Cancel) -> Self {
        Self { injector, cancel }
    }

    /// Push an item into the injector
    /// Returns `Err` if the injector has been cancelled
    pub fn push(&self, item: T) -> Result<(), ()> {
        self.injector.push(item.clone(), |dst| dst[0] = format!("{item}").into());
        if self.cancel.is_cancelled() { Err(()) } else { Ok(()) }
    }
}
