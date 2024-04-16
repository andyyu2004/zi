mod explorer;
mod inspector;
mod picker;
mod text;

use std::any::Any;
use std::fmt;
use std::path::{Path, PathBuf};

use crop::Rope;
use stdx::sync::Cancel;
use tree_sitter::QueryCursor;
use unicode_width::UnicodeWidthChar;

pub use self::explorer::ExplorerBuffer;
pub use self::inspector::InspectorBuffer;
pub use self::picker::{FilePicker, Picker, PickerBuffer};
pub use self::text::TextBuffer;
use crate::editor::{Resource, Selector, SyncClient};
use crate::keymap::Keymap;
use crate::private::Sealed;
use crate::syntax::{HighlightId, Syntax, Theme};
use crate::text::{AnyText, Delta};
use crate::{Editor, FileType, Point, Range, Size, Url, View};

slotmap::new_key_type! {
    pub struct BufferId;
}

impl Sealed for BufferId {}

impl Selector<Self> for BufferId {
    #[inline]
    fn select(&self, _: &Editor) -> Self {
        *self
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct BufferFlags: u8 {
        const READONLY = 0b0000_0001;
    }

    #[derive(Debug, Clone, Copy)]
    struct EditFlags: u8 {
        const PUSH_UNDO = 1 << 0;
        const NO_APPEND_NEWLINE = 1 << 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Highlight {
    pub range: Range,
    pub id: HighlightId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntaxHighlight {
    pub range: Range,
    pub id: HighlightId,
    pub capture_idx: u32,
}

impl Resource for dyn Buffer {
    type Id = BufferId;

    const URL_SCHEME: &'static str = "buffer";

    fn id(&self) -> Self::Id {
        self.id()
    }

    fn url(&self) -> &Url {
        let url = self.url();
        assert_eq!(url.scheme(), Self::URL_SCHEME);
        url
    }
}

// FIXME bad name, used for redo too
// Should also not be public (however needs to be because it's exposed by the buffer trait for now)
#[derive(Clone, Debug)]
pub struct UndoEntry {
    /// The saved cursor position
    pub cursor: Point,
    /// The delta that was applied
    pub delta: Delta<'static>,
    /// The delta that can be applied to undo the operation
    pub inversion: Delta<'static>,
}

pub trait Buffer {
    fn id(&self) -> BufferId;

    fn flags(&self) -> BufferFlags;

    fn path(&self) -> &Path;

    fn url(&self) -> &Url;

    fn file_url(&self) -> Option<&Url>;

    fn file_type(&self) -> &FileType;

    fn tab_width(&self) -> u8;

    fn text(&self) -> &dyn AnyText;

    fn version(&self) -> u32;

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn redo(&mut self) -> Option<UndoEntry> {
        None
    }

    fn undo(&mut self) -> Option<UndoEntry> {
        None
    }

    fn clear_undo(&mut self) {}

    /// Edit the buffer with a delta.
    /// The cursor is the current cursor position in the buffer as of the edit.
    fn edit(&mut self, cursor: Point, delta: &Delta<'_>);

    fn syntax(&self) -> Option<&Syntax> {
        None
    }

    /// Syntax highlights iterator.
    /// All ranges must be single-line ranges.
    fn syntax_highlights<'a>(
        &'a self,
        editor: &Editor,
        cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = SyntaxHighlight> + 'a> {
        let _ = (editor, cursor);
        Box::new(std::iter::empty())
    }

    /// Overlay highlights iterator that are merged with the syntax highlights.
    /// Overlay highlights take precedence.
    /// All ranges must be single-line ranges.
    fn overlay_highlights<'a>(
        &'a self,
        editor: &'a Editor,
        view: &View,
        size: Size,
    ) -> Box<dyn Iterator<Item = Highlight> + 'a> {
        let _ = (editor, view, size);
        Box::new(std::iter::empty())
    }

    fn boxed(self) -> Box<dyn Buffer + Send>
    where
        Self: Sized + Send + 'static,
    {
        Box::new(self)
    }

    fn keymap(&mut self) -> Option<&mut Keymap> {
        None
    }

    /// Called just before rendering the buffer
    fn pre_render(&mut self, _client: &SyncClient, view: &View, _area: tui::Rect) {
        assert_eq!(self.id(), view.buffer());
    }

    /// Called when a view is closed that was displaying this buffer
    fn on_leave(&mut self) {}

    fn char_width(&self, c: char) -> usize {
        c.width().unwrap_or(match c {
            '\t' => self.tab_width() as usize,
            _ => 0,
        })
    }
}

// NOTE: remember to add all the methods to the Box<dyn Buffer> impl below, including default methods
impl Buffer for Box<dyn Buffer + Send> {
    #[inline]
    fn id(&self) -> BufferId {
        self.as_ref().id()
    }

    #[inline]
    fn flags(&self) -> BufferFlags {
        self.as_ref().flags()
    }

    #[inline]
    fn path(&self) -> &Path {
        self.as_ref().path()
    }

    #[inline]
    fn url(&self) -> &Url {
        self.as_ref().url()
    }

    #[inline]
    fn file_url(&self) -> Option<&Url> {
        self.as_ref().file_url()
    }

    #[inline]
    fn file_type(&self) -> &FileType {
        self.as_ref().file_type()
    }

    #[inline]
    fn tab_width(&self) -> u8 {
        self.as_ref().tab_width()
    }

    #[inline]
    fn text(&self) -> &dyn AnyText {
        self.as_ref().text()
    }

    #[inline]
    fn version(&self) -> u32 {
        self.as_ref().version()
    }

    #[inline]
    fn syntax(&self) -> Option<&Syntax> {
        self.as_ref().syntax()
    }

    #[inline]
    fn as_any(&self) -> &dyn Any {
        self.as_ref().as_any()
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.as_mut().as_any_mut()
    }

    #[inline]
    fn edit(&mut self, cursor: Point, delta: &Delta<'_>) {
        self.as_mut().edit(cursor, delta)
    }

    #[inline]
    fn undo(&mut self) -> Option<UndoEntry> {
        self.as_mut().undo()
    }

    #[inline]
    fn redo(&mut self) -> Option<UndoEntry> {
        self.as_mut().redo()
    }

    #[inline]
    fn clear_undo(&mut self) {
        self.as_mut().clear_undo()
    }

    #[inline]
    fn syntax_highlights<'a>(
        &'a self,
        editor: &Editor,
        cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = SyntaxHighlight> + 'a> {
        self.as_ref().syntax_highlights(editor, cursor)
    }

    #[inline]
    fn overlay_highlights<'a>(
        &'a self,
        editor: &'a Editor,
        view: &View,
        size: Size,
    ) -> Box<dyn Iterator<Item = Highlight> + '_> {
        self.as_ref().overlay_highlights(editor, view, size)
    }

    #[inline]
    fn boxed(self) -> Box<dyn Buffer + Send>
    where
        Self: Sized + Send + 'static,
    {
        self
    }

    #[inline]
    fn keymap(&mut self) -> Option<&mut Keymap> {
        self.as_mut().keymap()
    }

    #[inline]
    fn pre_render(&mut self, sender: &SyncClient, view: &View, area: tui::Rect) {
        self.as_mut().pre_render(sender, view, area);
    }

    #[inline]
    fn on_leave(&mut self) {
        self.as_mut().on_leave();
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

#[cfg(test)]
mod tests;
