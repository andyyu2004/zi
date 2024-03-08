mod explorer;
mod picker;
mod text;

use std::any::Any;
use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};

use ropey::Rope;
use stdx::iter::BidirectionalIterator;
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

pub trait Text: fmt::Display {
    fn get_line(&self, line_idx: usize) -> Option<Cow<'_, str>>;
    fn get_char(&self, char_idx: usize) -> Option<char>;

    fn line_to_char(&self, line_idx: usize) -> usize;
    fn char_to_line(&self, char_idx: usize) -> usize;

    fn len_lines(&self) -> usize;
    fn len_chars(&self) -> usize;

    fn lines_at(&self, line_idx: usize) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_>;
    fn chars_at(&self, char_idx: usize) -> Box<dyn BidirectionalIterator<Item = char> + '_>;

    fn line(&self, line: usize) -> Cow<'_, str> {
        self.get_line(line).unwrap_or_else(|| {
            panic!("line out of bounds: {line}");
        })
    }
}

/// Some magic to match the behaviour of `[ropey::Ropey]`
fn str_lines(s: &str) -> impl Iterator<Item = Cow<'_, str>> + '_ {
    // append an empty line if the string ends with a newline or is empty (to match ropey's behaviour)
    s.split_inclusive('\n').chain((s.is_empty() || s.ends_with('\n')).then_some("")).map(Into::into)
}

/// Naive implementation of `Text` for `str`.
/// Most of these methods are O(n) and large strs should be avoided.
impl Text for str {
    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Cow<'_, str>> {
        str_lines(self).nth(line_idx)
    }

    #[inline]
    fn get_char(&self, char_idx: usize) -> Option<char> {
        self.chars().nth(char_idx)
    }

    #[inline]
    fn line_to_char(&self, line_idx: usize) -> usize {
        str_lines(self).take(line_idx).map(|l| l.len_chars()).sum()
    }

    #[inline]
    fn char_to_line(&self, mut char_idx: usize) -> usize {
        // This should be a real assert, but it's expensive so we just return the last line
        debug_assert!(char_idx < self.len_chars(), "char_idx out of bounds: {char_idx}");

        str_lines(self)
            .take_while(|l| {
                let n = l.len_chars();
                if n > char_idx {
                    return false;
                }
                char_idx -= n;
                true
            })
            .count()
    }

    #[inline]
    fn len_lines(&self) -> usize {
        1 + str_lines(self).filter(|line| line.ends_with('\n')).count()
    }

    #[inline]
    fn len_chars(&self) -> usize {
        self.chars().count()
    }

    #[inline]
    fn lines_at(&self, line_idx: usize) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        Box::new(str_lines(self).skip(line_idx))
    }

    #[inline]
    fn chars_at(&self, _char_idx: usize) -> Box<dyn BidirectionalIterator<Item = char> + '_> {
        todo!()
    }
}

impl Text for Rope {
    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Cow<'_, str>> {
        self.get_line(line_idx).map(Into::into)
    }

    fn get_char(&self, char_idx: usize) -> Option<char> {
        self.get_char(char_idx)
    }

    #[inline]
    fn line_to_char(&self, line_idx: usize) -> usize {
        self.line_to_char(line_idx)
    }

    #[inline]
    fn char_to_line(&self, char_idx: usize) -> usize {
        self.char_to_line(char_idx)
    }

    #[inline]
    fn len_lines(&self) -> usize {
        self.len_lines()
    }

    #[inline]
    fn len_chars(&self) -> usize {
        self.len_chars()
    }

    #[inline]
    fn lines_at(&self, line_idx: usize) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        Box::new(self.lines_at(line_idx).map(Into::into))
    }

    fn chars_at(&self, char_idx: usize) -> Box<dyn BidirectionalIterator<Item = char> + '_> {
        Box::new(self.chars_at(char_idx))
    }
}

pub trait Buffer {
    fn id(&self) -> BufferId;
    fn path(&self) -> &Path;
    fn url(&self) -> Option<Url>;
    fn language_id(&self) -> &FileType;
    fn tab_width(&self) -> u8;
    fn text(&self) -> &dyn Text;
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

    fn text(&self) -> &dyn Text {
        self.as_ref().text()
    }

    fn version(&self) -> u32 {
        self.as_ref().version()
    }

    fn as_any(&self) -> &dyn Any {
        self.as_ref().as_any()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.as_mut().as_any_mut()
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

    fn boxed(self) -> Box<dyn Buffer>
    where
        Self: Sized + 'static,
    {
        self
    }

    fn keymap(&mut self) -> Option<&mut Keymap> {
        self.as_mut().keymap()
    }

    fn pre_render(&mut self, view: &View, area: tui::Rect) {
        self.as_mut().pre_render(view, area);
    }

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
