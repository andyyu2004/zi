mod explorer;
mod inspector;
mod mark;
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
use self::mark::Marks;
pub use self::mark::{Mark, MarkBuilder, MarkId};
pub use self::picker::PickerBuffer;
pub use self::text::TextBuffer;
use crate::config::Setting;
use crate::editor::{Resource, Selector};
use crate::keymap::Keymap;
use crate::private::{Internal, Sealed};
use crate::syntax::{HighlightId, Syntax, Theme};
use crate::{Client, Editor, FileType, Point, PointRange, Size, Url, View};

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
        const ENSURE_TRAILING_NEWLINE = 1 << 2;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct SnapshotFlags: u8 {
        const ALLOW_EMPTY = 1 << 0;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct EditFlags: u8 {
        /// Do not record this change in the undo tree.
        const NO_RECORD = 1 << 0;
        const NO_ENSURE_TRAILING_NEWLINE = 1 << 1;
    }
}

/// Buffer local settings
pub struct Settings {
    pub tab_width: Setting<u8>,
    pub indent: Setting<IndentSettings>,
    pub format_on_save: Setting<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentSettings {
    Spaces(u8),
    Tabs,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tab_width: Setting::new(4),
            indent: Setting::new(IndentSettings::Spaces(4)),
            format_on_save: Setting::new(true),
        }
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

impl Resource for Buffer {
    type Id = BufferId;

    const URL_SCHEME: &'static str = "buffer";

    fn id(&self) -> Self::Id {
        self.inner.id()
    }

    fn url(&self) -> &Url {
        let url = self.inner.url();
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

pub(crate) trait BufferHistory {
    /// Return the next undo entry (without applying it)
    fn undo(&mut self) -> Option<UndoEntry>;

    /// Return the next redo entry (without applying it)
    fn redo(&mut self) -> Option<UndoEntry>;

    fn clear(&mut self);

    fn snapshot(&mut self, flags: SnapshotFlags);

    fn snapshot_cursor(&mut self, cursor: Point);
}

// This wraps the trait to provide common functionality and to make it easier to control method privacy.
pub struct Buffer {
    marks: Marks,
    inner: Box<dyn BufferInternal>,
}

impl Buffer {
    pub(crate) fn new(buffer: impl BufferInternal + 'static) -> Self {
        Self { inner: buffer.boxed(), marks: Default::default() }
    }

    pub fn id(&self) -> BufferId {
        self.inner.id()
    }

    pub fn version(&self) -> u32 {
        self.inner.version()
    }

    pub fn file_type(&self) -> FileType {
        self.inner.file_type()
    }

    pub fn flags(&self) -> BufferFlags {
        self.inner.flags()
    }

    pub fn text(&self) -> &(dyn AnyText + 'static) {
        self.inner.text()
    }

    pub fn file_url(&self) -> Option<&Url> {
        self.inner.file_url()
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.inner.path()
    }

    pub fn settings(&self) -> &Settings {
        self.inner.settings()
    }

    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        self.edit_flags(deltas, EditFlags::empty())
    }

    pub fn edit_flags(&mut self, deltas: &Deltas<'_>, flags: EditFlags) {
        // TODO adjust marks
        self.inner.edit_flags(Internal(()), deltas, flags);
    }

    pub(crate) fn keymap(&mut self) -> Option<&mut Keymap> {
        self.inner.keymap(Internal(()))
    }

    pub(crate) fn as_any(&self) -> &dyn Any {
        self.inner.as_any()
    }

    pub(crate) fn as_any_mut(&mut self, internal: Internal) -> &mut dyn Any {
        self.inner.as_any_mut(internal)
    }

    pub(crate) fn flushed(&mut self) {
        self.inner.flushed(Internal(()));
    }

    pub(crate) fn on_leave(&mut self) {
        self.inner.on_leave(Internal(()));
    }

    pub(crate) fn pre_render(&mut self, client: &Client, view: &View, area: tui::Rect) {
        self.inner.pre_render(Internal(()), client, view, area);
    }

    pub(crate) fn snapshot(&mut self, flags: SnapshotFlags) {
        self.inner.snapshot(flags);
    }

    pub(crate) fn snapshot_cursor(&mut self, cursor: Point) {
        self.inner.snapshot_cursor(cursor);
    }

    pub(crate) fn undo(&mut self) -> Option<UndoEntry> {
        self.inner.undo()
    }

    pub(crate) fn redo(&mut self) -> Option<UndoEntry> {
        self.inner.redo()
    }

    pub(crate) fn clear_undo(&mut self) {
        self.inner.clear_undo();
    }

    pub(crate) fn syntax_highlights<'a>(
        &'a self,
        editor: &Editor,
        cursor: &'a mut QueryCursor,
        range: PointRange,
    ) -> Box<dyn Iterator<Item = SyntaxHighlight> + 'a> {
        self.inner.syntax_highlights(editor, cursor, range)
    }

    pub(crate) fn overlay_highlights<'a>(
        &'a self,
        editor: &'a Editor,
        view: &View,
        size: Size,
    ) -> Box<dyn Iterator<Item = Highlight> + 'a> {
        self.inner.overlay_highlights(editor, view, size)
    }

    pub fn syntax(&self) -> Option<&Syntax> {
        self.inner.syntax()
    }

    pub(crate) fn char_width(&self, c: char) -> usize {
        c.width().unwrap_or(match c {
            '\t' => *self.settings().tab_width.read() as usize,
            _ => 0,
        })
    }
}

pub(crate) trait BufferInternal: Send + Sync {
    fn id(&self) -> BufferId;

    fn flags(&self) -> BufferFlags;

    fn url(&self) -> &Url;

    fn file_url(&self) -> Option<&Url>;

    fn file_type(&self) -> FileType;

    fn text(&self) -> &(dyn AnyText + 'static);

    fn settings(&self) -> &Settings;

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

    fn boxed(self) -> Box<dyn BufferInternal>
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
    fn pre_render(&mut self, _: Internal, _client: &Client, view: &View, _area: tui::Rect) {
        assert_eq!(self.id(), view.buffer());
    }

    /// Called when a view is closed that was displaying this buffer
    fn on_leave(&mut self, _: Internal) {}
}

impl dyn BufferInternal + '_ {
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
impl BufferInternal for Box<dyn BufferInternal> {
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
    fn settings(&self) -> &Settings {
        self.as_ref().settings()
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
    fn boxed(self) -> Box<dyn BufferInternal>
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
    fn pre_render(&mut self, internal: Internal, client: &Client, view: &View, area: tui::Rect) {
        self.as_mut().pre_render(internal, client, view, area)
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
