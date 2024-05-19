mod explorer;
mod inspector;
pub mod picker;
mod text;

use std::any::Any;
use std::fmt;
use std::path::{Path, PathBuf};

use stdx::sync::Cancel;
use tree_sitter::QueryCursor;
use unicode_width::UnicodeWidthChar;
use zi_text::{AnyText, Delta, Deltas};

pub use self::explorer::ExplorerBuffer;
pub use self::inspector::InspectorBuffer;
pub use self::picker::PickerBuffer;
pub use self::text::TextBuffer;
use crate::config::Setting;
use crate::editor::{Resource, Selector, SyncClient};
use crate::keymap::Keymap;
use crate::private::{Internal, Sealed};
use crate::syntax::{HighlightId, Syntax, Theme};
use crate::{Editor, FileType, Point, PointRange, Size, Url, View};

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
        const READONLY = 1 << 0;
        const DIRTY = 1 << 1;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct SnapshotFlags: u8 {
        const ALLOW_EMPTY = 1 << 0;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct EditFlags: u8 {
        /// Do not record this change in the undo tree.
        const NO_RECORD = 1 << 0;
        /// Ensure the buffer ends with a newline before any insert.
        const NO_ENSURE_NEWLINE = 1 << 1;
    }
}

/// Buffer local configuration
pub struct Config {
    pub tab_width: Setting<u8>,
    pub format_on_save: Setting<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self { tab_width: Setting::new(4), format_on_save: Setting::new(true) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Highlight {
    pub range: PointRange,
    pub id: HighlightId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntaxHighlight {
    pub range: PointRange,
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

#[derive(Clone, Debug)]
pub struct UndoEntry {
    pub changes: Box<[Change]>,
    pub cursor: Option<Point>,
}

#[derive(Clone, Debug)]
pub struct Change {
    /// The deltas that were applied
    pub deltas: Deltas<'static>,
    /// The deltas that can be applied to undo the operation
    pub inversions: Deltas<'static>,
}

pub trait BufferHistory {
    /// Return the next undo entry (without applying it)
    fn undo(&mut self) -> Option<UndoEntry>;

    /// Return the next redo entry (without applying it)
    fn redo(&mut self) -> Option<UndoEntry>;

    fn clear(&mut self);

    fn snapshot(&mut self, flags: SnapshotFlags);

    fn snapshot_cursor(&mut self, cursor: Point);
}

pub trait Buffer: Send {
    fn id(&self) -> BufferId;

    fn flags(&self) -> BufferFlags;

    fn url(&self) -> &Url;

    fn file_url(&self) -> Option<&Url>;

    fn file_type(&self) -> FileType;

    fn text(&self) -> &(dyn AnyText + 'static);

    fn config(&self) -> &Config;

    fn version(&self) -> u32;

    fn as_any(&self) -> &dyn Any;

    /// The path of the buffer, if it is backed by a file. This is derived from `file_url`
    fn path(&self) -> Option<PathBuf> {
        self.file_url().and_then(|url| url.to_file_path().ok())
    }

    #[doc(hidden)]
    fn as_any_mut(&mut self, _: Internal) -> &mut dyn Any;

    #[doc(hidden)]
    fn history_mut(&mut self, _: Internal) -> Option<&mut dyn BufferHistory> {
        None
    }

    /// Edit the buffer with a delta.
    #[doc(hidden)]
    fn edit_flags(&mut self, _: Internal, deltas: &Deltas<'_>, flags: EditFlags);

    fn edit(&mut self, _: Internal, deltas: &Deltas<'_>) {
        self.edit_flags(Internal(()), deltas, EditFlags::empty());
    }

    /// `flushed` is called after the current text has been successfully written to disk.
    /// The implementation should update the buffer's state to reflect this.
    #[doc(hidden)]
    fn flushed(&mut self, _: Internal);

    #[doc(hidden)]
    fn syntax(&self) -> Option<&Syntax> {
        None
    }

    /// Syntax highlights iterator.
    /// All ranges must be single-line ranges.
    /// The highlights must cover at least the given point range, it is valid to return
    /// highlights that extend beyond.
    // TODO we should pass in a byte or point range (currently known as DeltaRange)
    #[doc(hidden)]
    fn syntax_highlights<'a>(
        &'a self,
        editor: &Editor,
        cursor: &'a mut QueryCursor,
        range: PointRange,
    ) -> Box<dyn Iterator<Item = SyntaxHighlight> + 'a> {
        let _ = (editor, cursor, range);
        Box::new(std::iter::empty())
    }

    /// Overlay highlights iterator that are merged with the syntax highlights.
    /// Overlay highlights take precedence.
    /// All ranges must be single-line ranges.
    #[doc(hidden)]
    fn overlay_highlights<'a>(
        &'a self,
        editor: &'a Editor,
        view: &View,
        size: Size,
    ) -> Box<dyn Iterator<Item = Highlight> + 'a> {
        let _ = (editor, view, size);
        Box::new(std::iter::empty())
    }

    fn boxed(self) -> Box<dyn Buffer>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }

    #[doc(hidden)]
    fn keymap(&mut self, _: Internal) -> Option<&mut Keymap> {
        None
    }

    /// Called just before rendering the buffer, returns whether the buffer needs to be re-rendered.
    fn pre_render(&mut self, _: Internal, _client: &SyncClient, view: &View, _area: tui::Rect) {
        assert_eq!(self.id(), view.buffer());
    }

    /// Called when a view is closed that was displaying this buffer
    fn on_leave(&mut self, _: Internal) {}

    fn char_width(&self, c: char) -> usize {
        c.width().unwrap_or(match c {
            '\t' => *self.config().tab_width.read() as usize,
            _ => 0,
        })
    }
}

impl dyn Buffer + '_ {
    #[inline]
    pub(crate) fn redo(&mut self) -> Option<UndoEntry> {
        self.history_mut(Internal(())).and_then(|h| h.redo())
    }

    #[inline]
    pub(crate) fn undo(&mut self) -> Option<UndoEntry> {
        self.history_mut(Internal(())).and_then(|h| h.undo())
    }

    #[inline]
    pub(crate) fn clear_undo(&mut self) {
        if let Some(h) = self.history_mut(Internal(())) {
            h.clear();
        }
    }

    #[inline]
    pub(crate) fn snapshot(&mut self, flags: SnapshotFlags) {
        if let Some(h) = self.history_mut(Internal(())) {
            h.snapshot(flags)
        }
    }

    #[inline]
    pub(crate) fn snapshot_cursor(&mut self, cursor: Point) {
        if let Some(h) = self.history_mut(Internal(())) {
            h.snapshot_cursor(cursor)
        }
    }
}

// NOTE: remember to add all the methods to the Box<dyn Buffer> impl below, including default methods
impl Buffer for Box<dyn Buffer> {
    #[inline]
    fn id(&self) -> BufferId {
        self.as_ref().id()
    }

    #[inline]
    fn flags(&self) -> BufferFlags {
        self.as_ref().flags()
    }

    #[inline]
    fn flushed(&mut self, internal: Internal) {
        self.as_mut().flushed(internal);
    }

    #[inline]
    fn path(&self) -> Option<PathBuf> {
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
    fn file_type(&self) -> FileType {
        self.as_ref().file_type()
    }

    #[inline]
    fn config(&self) -> &Config {
        self.as_ref().config()
    }

    #[inline]
    fn text(&self) -> &(dyn AnyText + 'static) {
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
    fn as_any_mut(&mut self, internal: Internal) -> &mut dyn Any {
        self.as_mut().as_any_mut(internal)
    }

    #[inline]
    fn edit_flags(&mut self, internal: Internal, deltas: &Deltas<'_>, flags: EditFlags) {
        self.as_mut().edit_flags(internal, deltas, flags)
    }

    #[inline]
    fn history_mut(&mut self, internal: Internal) -> Option<&mut dyn BufferHistory> {
        self.as_mut().history_mut(internal)
    }

    #[inline]
    fn syntax_highlights<'a>(
        &'a self,
        editor: &Editor,
        cursor: &'a mut QueryCursor,
        range: PointRange,
    ) -> Box<dyn Iterator<Item = SyntaxHighlight> + 'a> {
        self.as_ref().syntax_highlights(editor, cursor, range)
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
    fn boxed(self) -> Box<dyn Buffer>
    where
        Self: Sized + 'static,
    {
        self
    }

    #[inline]
    fn keymap(&mut self, internal: Internal) -> Option<&mut Keymap> {
        self.as_mut().keymap(internal)
    }

    #[inline]
    fn pre_render(
        &mut self,
        internal: Internal,
        sender: &SyncClient,
        view: &View,
        area: tui::Rect,
    ) {
        self.as_mut().pre_render(internal, sender, view, area)
    }

    #[inline]
    fn on_leave(&mut self, internal: Internal) {
        self.as_mut().on_leave(internal);
    }
}

pub trait Entry: fmt::Display + fmt::Debug + Clone + Sync + Send + 'static {}

impl<T> Entry for T where T: fmt::Display + fmt::Debug + Clone + Sync + Send + 'static {}

/// Wrapper around a `nucleo::Injector` with cancellation support
#[derive(Clone)]
pub struct Injector<T> {
    injector: nucleo::Injector<T>,
    cancel: Cancel,
}

impl<T: Entry> Injector<T> {
    pub fn new(injector: nucleo::Injector<T>, cancel: Cancel) -> Self {
        Self { injector, cancel }
    }

    /// Push an item into the injector
    /// Returns `Err` if the injector has been cancelled
    pub(crate) fn push(&self, item: T) -> Result<(), ()> {
        self.injector.push(item.clone(), |_, dst| dst[0] = format!("{item}").into());
        if self.cancel.is_cancelled() { Err(()) } else { Ok(()) }
    }
}

#[cfg(test)]
mod tests;
