mod delta;
mod explorer;
mod picker;
mod readonly;
mod text;

use std::any::Any;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::slice::SliceIndex;
use std::{fmt, ops};

use ropey::Rope;
use stdx::iter::BidirectionalIterator;
use stdx::sync::Cancel;
use tree_sitter::QueryCursor;

pub use self::delta::Delta;
use self::delta::DeltaRange;
pub use self::explorer::ExplorerBuffer;
pub use self::picker::{FilePicker, Picker, PickerBuffer};
pub use self::readonly::ReadonlyText;
pub use self::text::TextBuffer;
use crate::editor::{Resource, SyncClient};
use crate::keymap::Keymap;
use crate::syntax::{HighlightId, HighlightMap, Highlights, Syntax, Theme};
use crate::{FileType, Line, Point, Range, Size, Url, View};

slotmap::new_key_type! {
    pub struct BufferId;
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct BufferFlags: u32 {
        const READONLY = 0b0000_0001;
    }
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

pub trait TextMut: Text {
    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error>;
}

impl TextMut for Rope {
    #[inline]
    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error> {
        let range = self.delta_to_char_range(delta);
        let start = range.start;
        self.try_remove(range)?;
        self.try_insert(start, delta.text())
    }
}

/// These methods are in a separate trait because these methods are not lazy and require reading the entire file.
/// Avoid their use if possible.
pub trait Text: LazyText {
    fn len_lines(&self) -> usize;

    fn len_chars(&self) -> usize;

    fn len_bytes(&self) -> usize;
}

pub trait LazyText: fmt::Display {
    fn get_line(&self, line_idx: usize) -> Option<Cow<'_, str>>;
    fn get_char(&self, char_idx: usize) -> Option<char>;

    fn line_to_char(&self, line_idx: usize) -> usize;
    fn char_to_line(&self, char_idx: usize) -> usize;

    fn byte_to_line(&self, byte_idx: usize) -> usize;
    fn line_to_byte(&self, line_idx: usize) -> usize;

    fn char_to_byte(&self, char_idx: usize) -> usize;

    fn lines_at(&self, line_idx: usize) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_>;
    fn chars_at(&self, char_idx: usize) -> Box<dyn BidirectionalIterator<Item = char> + '_>;

    fn chunk_at_byte(&self, byte_idx: usize) -> &str;

    fn byte_slice(&self, range: std::ops::Range<usize>) -> Box<dyn Iterator<Item = &str> + '_>;

    fn as_text(&self) -> Option<&dyn Text>;

    fn as_text_mut(&mut self) -> Option<&mut dyn TextMut>;

    fn byte_to_point(&self, byte_idx: usize) -> Point {
        let line_idx = self.byte_to_line(byte_idx);
        Point::new(line_idx, byte_idx - self.line_to_byte(line_idx))
    }

    #[inline]
    fn char_to_point(&self, char_idx: usize) -> Point {
        let line = self.char_to_line(char_idx);
        let col = char_idx - self.line_to_char(line);
        Point::new(line, col)
    }

    #[inline]
    fn delta_to_point_range(&self, delta: &Delta<'_>) -> Range {
        match delta.range() {
            DeltaRange::Point(p) => p,
            DeltaRange::Char(range) => {
                Range::new(self.char_to_point(range.start), self.char_to_point(range.end))
            }
        }
    }

    #[inline]
    fn delta_to_char_range(&self, delta: &Delta<'_>) -> ops::Range<usize> {
        match delta.range() {
            DeltaRange::Point(range) => self.point_range_to_char_range(range),
            DeltaRange::Char(range) => range,
        }
    }

    #[inline]
    fn delta_to_byte_range(&self, delta: &Delta<'_>) -> ops::Range<usize> {
        match delta.range() {
            DeltaRange::Point(range) => self.point_range_to_byte_range(range),
            DeltaRange::Char(range) => self.char_range_to_byte_range(range),
        }
    }

    #[inline]
    fn line_in_bounds(&self, line: usize) -> bool {
        self.get_line(line).is_some()
    }

    #[inline]
    fn point_to_char(&self, point: Point) -> usize {
        self.line_to_char(point.line().idx()) + point.col().idx()
    }

    #[inline]
    fn point_to_byte(&self, point: Point) -> usize {
        self.char_to_byte(self.point_to_char(point))
    }

    #[inline]
    fn point_range_to_char_range(&self, range: Range) -> ops::Range<usize> {
        self.point_to_char(range.start())..self.point_to_char(range.end())
    }

    #[inline]
    fn char_range_to_byte_range(&self, range: ops::Range<usize>) -> ops::Range<usize> {
        self.char_to_byte(range.start)..self.char_to_byte(range.end)
    }

    #[inline]
    fn point_range_to_byte_range(&self, range: Range) -> ops::Range<usize> {
        self.point_to_byte(range.start())..self.point_to_byte(range.end())
    }

    #[inline]
    fn lines(&self) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        self.lines_at(0)
    }

    #[inline]
    fn line(&self, line: usize) -> Cow<'_, str> {
        self.get_line(line).unwrap_or_else(|| {
            panic!("line out of bounds: {line}");
        })
    }

    fn annotate<'a, T: Copy>(
        &'a self,
        highlights: impl IntoIterator<Item = (Range, T)> + 'a,
    ) -> impl Iterator<Item = (Line, Cow<'a, str>, Option<T>)> + 'a
    where
        Self: Sized,
    {
        annotate(self.lines(), highlights)
    }
}

/// The returned lines are guaranteed to be single-line
pub fn annotate<'a, T: Copy>(
    lines: impl Iterator<Item = Cow<'a, str>> + 'a,
    annotations: impl IntoIterator<Item = (Range, T)> + 'a,
) -> impl Iterator<Item = (Line, Cow<'a, str>, Option<T>)> + 'a {
    // A specialized slice that preserves the borrow if possible
    fn slice_cow<'a, I: SliceIndex<str, Output = str>>(s: &Cow<'a, str>, index: I) -> Cow<'a, str> {
        match s {
            Cow::Borrowed(s) => Cow::Borrowed(&s[index]),
            Cow::Owned(s) => Cow::Owned(s[index].to_owned()),
        }
    }

    let mut annotations = annotations.into_iter().peekable();
    std::iter::from_coroutine(move || {
        for (i, line) in lines.enumerate() {
            let line_idx = Line::from(i);
            let mut j = 0;
            while let Some(&(range, annotation)) = annotations.peek() {
                if range.start().line() > i {
                    break;
                }

                let start_col =
                    if range.start().line() == i { range.start().col().idx() } else { 0 };

                if range.end().line() > i {
                    // If the highlight is a multi-line highlight,
                    // we style the entire line with that style and move on to highlight the next
                    // line (without next()ing the highlight iterator)
                    yield (line_idx, line.clone(), Some(annotation));
                    // set `j` here so we don't try to highlight the same range again
                    j = line.len();
                    break;
                }

                let (range, annotation) = annotations.next().expect("just peeked");
                let end_col = if range.end().line().idx() == i {
                    range.end().col().idx()
                } else {
                    line.len()
                };

                if start_col < j {
                    // Sometimes annotations can overlap, we just arbitrarily use the first one of that range
                    continue;
                }

                if start_col > j {
                    yield (line_idx, slice_cow(&line, j..start_col), None)
                }

                if end_col >= line.len() {
                    // We're allowed to annotate places with no text, so the range end might be out of bounds
                    // In which case, we add another span with the remaining space.

                    // There's a bit of a bug here:
                    // If the line ends with a newline, then the padded span will be on the next line.
                    // The workaround is to return the line number as well, so the renderer can handle it.
                    yield (line_idx, slice_cow(&line, start_col..), Some(annotation));
                    yield (
                        line_idx,
                        format!("{:width$}", "", width = end_col - line.len()).into(),
                        Some(annotation),
                    )
                } else {
                    yield (line_idx, slice_cow(&line, start_col..end_col), Some(annotation));
                }

                j = end_col;
            }

            // Add in a span for the rest of the line that wasn't annotated
            if j < line.len() {
                yield (line_idx, slice_cow(&line, j..), None);
            }
        }
    })
    // fuse the iterator avoid panics due to misuse
    .fuse()
    .filter(|(_, text, _)| !text.is_empty())
}

/// Some magic to match the behaviour of `[ropey::Ropey]`
fn str_lines(s: &str) -> impl Iterator<Item = Cow<'_, str>> + '_ {
    // append an empty line if the string ends with a newline or is empty (to match ropey's behaviour)
    s.split_inclusive('\n').chain((s.is_empty() || s.ends_with('\n')).then_some("")).map(Into::into)
}

impl Text for str {
    #[inline]
    fn len_bytes(&self) -> usize {
        self.len()
    }

    #[inline]
    fn len_lines(&self) -> usize {
        1 + str_lines(self).filter(|line| line.ends_with('\n')).count()
    }

    #[inline]
    fn len_chars(&self) -> usize {
        self.chars().count()
    }
}

/// Naive implementation of [`LazyText`] for `str`.
/// Most of these methods are O(n) and large strs should be avoided.
impl LazyText for str {
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
        str_lines(self).take(line_idx).map(|l| l.chars().count()).sum()
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        str_lines(self).take(line_idx).map(|l| l.len()).sum()
    }

    fn byte_to_line(&self, mut byte_idx: usize) -> usize {
        assert!(byte_idx <= self.len(), "byte_idx out of bounds: {byte_idx}");
        str_lines(self)
            .take_while(|l| {
                if l.len() > byte_idx {
                    return false;
                }
                byte_idx -= l.len();
                true
            })
            .count()
    }

    #[inline]
    fn char_to_line(&self, mut char_idx: usize) -> usize {
        // This should be a real assert, but it's expensive so we just return the last line
        // debug_assert!(char_idx < self.len_chars(), "char_idx out of bounds: {char_idx}");

        str_lines(self)
            .take_while(|l| {
                let n = l.chars().count();
                if n > char_idx {
                    return false;
                }
                char_idx -= n;
                true
            })
            .count()
    }

    #[inline]
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.chars().take(char_idx).map(|c| c.len_utf8()).sum()
    }

    #[inline]
    fn lines_at(&self, line_idx: usize) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        Box::new(str_lines(self).skip(line_idx))
    }

    #[inline]
    fn chars_at(&self, _char_idx: usize) -> Box<dyn BidirectionalIterator<Item = char> + '_> {
        todo!()
    }

    #[inline]
    fn chunk_at_byte(&self, byte_idx: usize) -> &str {
        &self[byte_idx..]
    }

    #[inline]
    fn byte_slice(&self, range: ops::Range<usize>) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(std::iter::once(&self[range]))
    }

    #[inline]
    fn as_text(&self) -> Option<&dyn Text> {
        // Not sure how to make this work as we're already a fat pointer
        // Also can't do `Some(&self)` since that's a reference to a local
        None
    }

    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn TextMut> {
        None
    }
}

impl Text for Rope {
    #[inline]
    fn len_bytes(&self) -> usize {
        self.len_bytes()
    }

    #[inline]
    fn len_lines(&self) -> usize {
        self.len_lines()
    }

    #[inline]
    fn len_chars(&self) -> usize {
        self.len_chars()
    }
}

impl LazyText for Rope {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn TextMut> {
        Some(self)
    }

    #[inline]
    fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.byte_to_line(byte_idx)
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.line_to_byte(line_idx)
    }

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
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.char_to_byte(char_idx)
    }

    #[inline]
    fn lines_at(&self, line_idx: usize) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        Box::new(self.lines_at(line_idx).map(Into::into))
    }

    #[inline]
    fn chars_at(&self, char_idx: usize) -> Box<dyn BidirectionalIterator<Item = char> + '_> {
        Box::new(self.chars_at(char_idx))
    }

    #[inline]
    fn chunk_at_byte(&self, byte_idx: usize) -> &str {
        let (chunk, start_byte, _, _) = self.chunk_at_byte(byte_idx);
        &chunk[byte_idx - start_byte..]
    }

    #[inline]
    fn byte_slice(&self, range: std::ops::Range<usize>) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(self.byte_slice(range).chunks())
    }

    #[inline]
    fn as_text(&self) -> Option<&dyn Text> {
        Some(self)
    }
}

pub trait Buffer {
    fn id(&self) -> BufferId;
    fn flags(&self) -> BufferFlags;
    fn path(&self) -> &Path;
    fn url(&self) -> &Url;
    fn file_url(&self) -> Option<&Url>;
    fn language_id(&self) -> &FileType;
    fn tab_width(&self) -> u8;
    fn text(&self) -> &dyn Text;
    fn version(&self) -> u32;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error>;

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
    fn language_id(&self) -> &FileType {
        self.as_ref().language_id()
    }

    #[inline]
    fn tab_width(&self) -> u8 {
        self.as_ref().tab_width()
    }

    #[inline]
    fn text(&self) -> &dyn Text {
        self.as_ref().text()
    }

    #[inline]
    fn version(&self) -> u32 {
        self.as_ref().version()
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
    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error> {
        self.as_mut().edit(delta)
    }

    #[inline]
    fn syntax_highlights<'a>(
        &'a self,
        cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + 'a> {
        self.as_ref().syntax_highlights(cursor)
    }

    #[inline]
    fn overlay_highlights(
        &self,
        view: &View,
        size: Size,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + '_> {
        self.as_ref().overlay_highlights(view, size)
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

impl<T: Text + ?Sized> Text for &T {
    #[inline]
    fn len_bytes(&self) -> usize {
        (**self).len_bytes()
    }

    #[inline]
    fn len_lines(&self) -> usize {
        (**self).len_lines()
    }

    #[inline]
    fn len_chars(&self) -> usize {
        (**self).len_chars()
    }
}

impl<T: LazyText + ?Sized> LazyText for &T {
    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Cow<'_, str>> {
        (**self).get_line(line_idx)
    }

    #[inline]
    fn get_char(&self, char_idx: usize) -> Option<char> {
        (**self).get_char(char_idx)
    }

    #[inline]
    fn line_to_char(&self, line_idx: usize) -> usize {
        (**self).line_to_char(line_idx)
    }

    #[inline]
    fn char_to_line(&self, char_idx: usize) -> usize {
        (**self).char_to_line(char_idx)
    }

    #[inline]
    fn byte_to_line(&self, byte_idx: usize) -> usize {
        (**self).byte_to_line(byte_idx)
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        (**self).line_to_byte(line_idx)
    }

    #[inline]
    fn char_to_byte(&self, char_idx: usize) -> usize {
        (**self).char_to_byte(char_idx)
    }

    #[inline]
    fn lines_at(&self, line_idx: usize) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        (**self).lines_at(line_idx)
    }

    #[inline]
    fn chars_at(&self, char_idx: usize) -> Box<dyn BidirectionalIterator<Item = char> + '_> {
        (**self).chars_at(char_idx)
    }

    #[inline]
    fn chunk_at_byte(&self, byte_idx: usize) -> &str {
        (**self).chunk_at_byte(byte_idx)
    }

    #[inline]
    fn byte_slice(&self, range: std::ops::Range<usize>) -> Box<dyn Iterator<Item = &str> + '_> {
        (**self).byte_slice(range)
    }

    #[inline]
    fn as_text(&self) -> Option<&dyn Text> {
        (**self).as_text()
    }

    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn TextMut> {
        None
    }

    #[inline]
    fn lines(&self) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        (**self).lines()
    }

    #[inline]
    fn line(&self, line: usize) -> Cow<'_, str> {
        (**self).line(line)
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
