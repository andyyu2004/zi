#![feature(
    iter_from_coroutine,
    trait_upcasting,
    coroutines,
    type_alias_impl_trait,
    impl_trait_in_assoc_type,
    array_chunks
)]

mod cow_str_impl;
mod cursor;
mod delta;
mod mark;
mod readonly;
mod rope;
mod str_impl;

use std::any::Any;
use std::borrow::Cow;
use std::io::Read;
use std::ops::{Bound, RangeBounds};
use std::{fmt, iter, ops};

pub use crop::{Rope, RopeBuilder, RopeSlice};
pub use cursor::RopeCursor;
use dyn_clone::DynClone;
use zi_core::{Line, Point, PointOrByte, PointRange};

pub use self::delta::{Delta, DeltaRange, Deltas};
pub use self::mark::{Bias, MTree, MarkTree};
pub use self::readonly::ReadonlyText;

/// Text that can be modified.
/// Required to be cloneable to store snapshots in the undo tree.
pub trait TextMut: Text {
    fn edit(&mut self, deltas: &Deltas<'_>) -> Deltas<'static>;
}

pub trait AnyTextMut: AnyText + Send + 'static {
    fn dyn_edit(&mut self, deltas: &Deltas<'_>) -> Deltas<'static>;

    fn as_text(&self) -> &dyn AnyText;
}

impl TextMut for dyn AnyTextMut + '_ {
    #[inline]
    fn edit(&mut self, delta: &Deltas<'_>) -> Deltas<'static> {
        self.dyn_edit(delta)
    }
}

impl<T: AnyText + TextMut + Send + 'static> AnyTextMut for T {
    #[inline]
    fn as_text(&self) -> &dyn AnyText {
        self
    }

    #[inline]
    fn dyn_edit(&mut self, deltas: &Deltas<'_>) -> Deltas<'static> {
        <T as TextMut>::edit(self, deltas)
    }
}

/// dyn-safe interface for reading text
pub trait TextBase: fmt::Display + fmt::Debug + Send + Sync {
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut>;

    /// Returns the number of lines in the text.
    /// This must be consistent with `self.lines.count()` if `Text` is also implemented
    fn len_lines(&self) -> usize;
    fn len_bytes(&self) -> usize;

    fn byte_to_line(&self, byte_idx: usize) -> usize;
    fn line_to_byte(&self, line_idx: usize) -> usize;

    fn byte_to_utf16_cu(&self, byte_idx: usize) -> usize;
    fn utf16_cu_to_byte(&self, cu_idx: usize) -> usize;

    fn get_char(&self, byte_idx: usize) -> Option<char>;

    fn try_line_to_byte(&self, line_idx: usize) -> Option<usize> {
        if line_idx < self.len_lines() { Some(self.line_to_byte(line_idx)) } else { None }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.len_bytes() == 0
    }

    #[inline]
    fn byte_to_point(&self, byte_idx: usize) -> Point {
        let line_idx = self.byte_to_line(byte_idx);
        Point::new(line_idx, byte_idx - self.line_to_byte(line_idx))
    }

    #[inline]
    fn point_to_byte(&self, point: Point) -> usize {
        // This doesn't actually check whether the column is within bounds of the line
        self.line_to_byte(point.line()) + point.col()
    }

    #[inline]
    fn point_range_to_byte_range(&self, range: PointRange) -> ops::Range<usize> {
        self.point_to_byte(range.start())..self.point_to_byte(range.end())
    }

    #[inline]
    fn byte_range_to_point_range(&self, range: &ops::Range<usize>) -> PointRange {
        PointRange::new(self.byte_to_point(range.start), self.byte_to_point(range.end))
    }

    #[inline]
    fn point_or_byte_to_byte(&self, point_or_byte: PointOrByte) -> usize {
        match point_or_byte {
            PointOrByte::Point(p) => self.point_to_byte(p),
            PointOrByte::Byte(b) => b,
        }
    }

    #[inline]
    fn point_or_byte_to_point(&self, point_or_byte: PointOrByte) -> Point {
        match point_or_byte {
            PointOrByte::Point(p) => p,
            PointOrByte::Byte(b) => self.byte_to_point(b),
        }
    }
}

impl<T: TextBase + ?Sized> TextBase for Box<T> {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        (**self).as_text_mut()
    }

    #[inline]
    fn len_lines(&self) -> usize {
        (**self).len_lines()
    }

    #[inline]
    fn len_bytes(&self) -> usize {
        (**self).len_bytes()
    }

    #[inline]
    fn byte_to_line(&self, byte_idx: usize) -> usize {
        (**self).byte_to_line(byte_idx)
    }

    #[inline]
    fn byte_to_utf16_cu(&self, byte_idx: usize) -> usize {
        (**self).byte_to_utf16_cu(byte_idx)
    }

    #[inline]
    fn utf16_cu_to_byte(&self, cu_idx: usize) -> usize {
        (**self).utf16_cu_to_byte(cu_idx)
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        (**self).line_to_byte(line_idx)
    }

    #[inline]
    fn get_char(&self, byte_idx: usize) -> Option<char> {
        (**self).get_char(byte_idx)
    }

    #[inline]
    fn try_line_to_byte(&self, line_idx: usize) -> Option<usize> {
        (**self).try_line_to_byte(line_idx)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        (**self).is_empty()
    }

    #[inline]
    fn byte_to_point(&self, byte_idx: usize) -> Point {
        (**self).byte_to_point(byte_idx)
    }

    #[inline]
    fn point_to_byte(&self, point: Point) -> usize {
        (**self).point_to_byte(point)
    }

    #[inline]
    fn point_range_to_byte_range(&self, range: PointRange) -> ops::Range<usize> {
        (**self).point_range_to_byte_range(range)
    }

    #[inline]
    fn point_or_byte_to_byte(&self, point_or_byte: PointOrByte) -> usize {
        (**self).point_or_byte_to_byte(point_or_byte)
    }

    #[inline]
    fn point_or_byte_to_point(&self, point_or_byte: PointOrByte) -> Point {
        (**self).point_or_byte_to_point(point_or_byte)
    }
}

impl<'a, T: TextSlice<'a>> PartialEq<T> for dyn AnyTextSlice<'a> {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        self.to_string() == other.to_string()
    }
}

pub trait AnyTextSlice<'a>: TextBase {
    fn dyn_to_cow(&self) -> Cow<'a, str>;
    fn dyn_byte_slice(
        &self,
        byte_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'a> + 'a>;
    fn dyn_line_slice(
        &self,
        line_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'a> + 'a>;
    fn dyn_chars(&self) -> Box<dyn DoubleEndedIterator<Item = char> + 'a>;
    fn dyn_lines(&self)
    -> Box<dyn DoubleEndedIterator<Item = Box<dyn AnyTextSlice<'a> + 'a>> + 'a>;
    fn dyn_chunks(&self) -> Box<dyn DoubleEndedIterator<Item = &'a str> + 'a>;
    fn dyn_line(&self, line_idx: usize) -> Option<Box<dyn AnyTextSlice<'a> + 'a>>;
}

dyn_clone::clone_trait_object!(AnyText);

pub trait AnyText: DynClone + TextBase + fmt::Display {
    fn dyn_byte_slice(
        &self,
        byte_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'_> + '_>;

    fn dyn_line_slice(
        &self,
        line_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'_> + '_>;

    fn dyn_chars(&self) -> Box<dyn DoubleEndedIterator<Item = char> + '_>;

    fn dyn_lines(&self)
    -> Box<dyn DoubleEndedIterator<Item = Box<dyn AnyTextSlice<'_> + '_>> + '_>;

    fn dyn_line(&self, line_idx: usize) -> Option<Box<dyn AnyTextSlice<'_> + '_>>;

    fn dyn_reader(&self) -> Box<dyn Read + Send + '_>;

    fn as_boxed_any(self: Box<Self>) -> Box<dyn Any>
    where
        Self: 'static;
}

impl<'a> TextSlice<'a> for &'a dyn AnyTextSlice<'a> {
    type Slice = Box<dyn AnyTextSlice<'a> + 'a>;
    type Chunks = Box<dyn DoubleEndedIterator<Item = &'a str> + 'a>;

    fn to_cow(&self) -> Cow<'a, str> {
        self.dyn_to_cow()
    }

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Box<dyn AnyTextSlice<'a> + 'a> {
        self.dyn_byte_slice((byte_range.start_bound().cloned(), byte_range.end_bound().cloned()))
    }

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice {
        self.dyn_line_slice((line_range.start_bound().cloned(), line_range.end_bound().cloned()))
    }

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + 'a {
        self.dyn_chars()
    }

    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice> + 'a {
        self.dyn_lines()
    }

    fn chunks(&self) -> Self::Chunks {
        self.dyn_chunks()
    }

    fn line(&self, line_idx: usize) -> Option<Self::Slice> {
        self.dyn_line(line_idx)
    }
}

impl Text for dyn AnyTextMut + '_ {
    type Slice<'a> = Box<dyn AnyTextSlice<'a> + 'a>
    where
        Self: 'a;

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        (self as &dyn AnyText).line_slice(line_range)
    }

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        (self as &dyn AnyText).byte_slice(byte_range)
    }

    fn line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        (self as &dyn AnyText).line(line_idx)
    }

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        (self as &dyn AnyText).chars()
    }

    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice<'_>> {
        (self as &dyn AnyText).lines()
    }

    fn reader(&self) -> impl Read + '_ {
        (self as &dyn AnyText).reader()
    }
}

impl Text for dyn AnyText + '_ {
    type Slice<'a> = Box<dyn AnyTextSlice<'a> + 'a>
    where
        Self: 'a;

    #[inline]
    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        self.dyn_line_slice((line_range.start_bound().cloned(), line_range.end_bound().cloned()))
    }

    #[inline]
    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        self.dyn_byte_slice((byte_range.start_bound().cloned(), byte_range.end_bound().cloned()))
    }

    #[inline]
    fn line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.dyn_line(line_idx)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        self.dyn_chars()
    }

    #[inline]
    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice<'_>> {
        self.dyn_lines()
    }

    #[inline]
    fn reader(&self) -> impl Read + Send + '_ {
        self.dyn_reader()
    }
}

impl<T: Text + Clone> AnyText for T {
    #[inline]
    fn dyn_byte_slice(
        &self,
        byte_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'_> + '_> {
        Box::new(self.byte_slice(byte_range))
    }

    #[inline]
    fn dyn_line_slice(
        &self,
        line_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'_> + '_> {
        Box::new(self.line_slice(line_range))
    }

    #[inline]
    fn dyn_chars(&self) -> Box<dyn DoubleEndedIterator<Item = char> + '_> {
        Box::new(self.chars())
    }

    #[inline]
    fn dyn_lines(
        &self,
    ) -> Box<dyn DoubleEndedIterator<Item = Box<dyn AnyTextSlice<'_> + '_>> + '_> {
        Box::new(self.lines().map(|s| Box::new(s) as _))
    }

    #[inline]
    fn dyn_line(&self, line_idx: usize) -> Option<Box<dyn AnyTextSlice<'_> + '_>> {
        self.line(line_idx).map(|s| Box::new(s) as _)
    }

    #[inline]
    fn as_boxed_any(self: Box<Self>) -> Box<dyn Any>
    where
        Self: 'static,
    {
        self
    }

    #[inline]
    fn dyn_reader(&self) -> Box<dyn Read + Send + '_> {
        Box::new(self.reader())
    }
}

impl<'a, T> AnyTextSlice<'a> for T
where
    T: TextSlice<'a> + 'a,
{
    fn dyn_to_cow(&self) -> Cow<'a, str> {
        <T as TextSlice<'a>>::to_cow(self)
    }

    fn dyn_byte_slice(
        &self,
        byte_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'a> + 'a> {
        Box::new(<T as TextSlice<'a>>::byte_slice(self, byte_range))
    }

    fn dyn_line_slice(
        &self,
        line_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'a> + 'a> {
        Box::new(<T as TextSlice<'a>>::byte_slice(self, line_range))
    }

    fn dyn_chars(&self) -> Box<dyn DoubleEndedIterator<Item = char> + 'a> {
        Box::new(<T as TextSlice<'a>>::chars(self))
    }

    fn dyn_lines(
        &self,
    ) -> Box<dyn DoubleEndedIterator<Item = Box<dyn AnyTextSlice<'a> + 'a>> + 'a> {
        Box::new(<T as TextSlice<'a>>::lines(self).map(|s| Box::new(s) as _))
    }

    fn dyn_chunks(&self) -> Box<dyn DoubleEndedIterator<Item = &'a str> + 'a> {
        Box::new(<T as TextSlice<'a>>::chunks(self))
    }

    fn dyn_line(&self, line_idx: usize) -> Option<Box<dyn AnyTextSlice<'a> + 'a>> {
        <T as TextSlice<'a>>::line(self, line_idx).map(move |s| Box::new(s) as _)
    }
}

/// Similar to [`Text`] except the returned lifetimes are tied to `'a` instead of `'self`.
pub trait TextSlice<'a>: TextBase + Sized {
    type Slice: TextSlice<'a>;

    /// There are some rust limitations/bugs when using `impl Trait` returns. We need to ensure the
    /// returned iterators are not tied to the lifetime of self. `impl Trait` currently seems to
    /// capture ths `'self` lifetime which is too limiting.
    /// All the other methods should be changed to but not hitting the issue yet.
    type Chunks: DoubleEndedIterator<Item = &'a str> + 'a;

    fn to_cow(&self) -> Cow<'a, str>;

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice;

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice;

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + 'a;

    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice> + 'a;

    fn chunks(&self) -> Self::Chunks;

    fn line(&self, line_idx: usize) -> Option<Self::Slice>;

    /// Returns the byte index of the first non-whitespace character on the line.
    #[inline]
    fn indent(&self, line_idx: usize) -> usize {
        self.line(line_idx)
            .expect("line index out of bounds (indent_bytes)")
            .chars()
            .take_while(|c| c.is_whitespace())
            .map(|c| c.len_utf8())
            .sum::<usize>()
    }

    /// Returns true if the point is within the indentation of the line.
    #[inline]
    fn inindent(&self, point: Point) -> bool {
        self.indent(point.line()) >= point.col()
    }

    fn reader(&self) -> impl Read + 'a {
        TextReader::new(self.chunks())
    }

    fn annotate<T: Copy>(
        &self,
        highlights: impl IntoIterator<Item = (PointRange, T)> + 'a,
    ) -> impl Iterator<Item = (Line, Cow<'a, str>, Option<T>)> + 'a
    where
        Self: Sized,
    {
        annotate(self.lines(), highlights)
    }
}

pub trait Text: TextBase {
    type Slice<'a>: TextSlice<'a>
    where
        Self: 'a;

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice<'_>;

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice<'_>;

    fn chars(&self) -> impl DoubleEndedIterator<Item = char>;

    /// Returns an iterator over the lines of the text.
    /// Each line should not include the newline character(s).
    /// This must always return at least one line, even if the text is empty.
    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice<'_>>;

    /// Returns the line at the given index excluding the newline character(s).
    fn line(&self, line_idx: usize) -> Option<Self::Slice<'_>>;

    fn reader(&self) -> impl Read + Send + '_;

    #[inline]
    fn char_at_point(&self, point: Point) -> Option<char> {
        self.char_at_byte(self.point_to_byte(point))
    }

    #[inline]
    fn char_at_byte(&self, byte_idx: usize) -> Option<char> {
        self.byte_slice(byte_idx..).chars().next()
    }

    #[inline]
    fn char_before_byte(&self, byte_idx: usize) -> Option<char> {
        self.byte_slice(..byte_idx).chars().next_back()
    }

    #[inline]
    fn char_before_point(&self, point: Point) -> Option<char> {
        self.char_before_byte(self.point_to_byte(point))
    }

    /// Returns the byte index of the first non-whitespace character on the line.
    #[inline]
    fn indent(&self, line_idx: usize) -> usize {
        self.byte_slice(..).indent(line_idx)
    }

    /// Returns true if the point is within the indentation of the line.
    #[inline]
    fn inindent(&self, point: Point) -> bool {
        self.byte_slice(..).inindent(point)
    }

    fn annotate<'a, T: Copy>(
        &'a self,
        highlights: impl IntoIterator<Item = (PointRange, T)> + 'a,
    ) -> impl Iterator<Item = (Line, Cow<'a, str>, Option<T>)> + 'a
    where
        Self: Sized,
    {
        annotate(self.lines(), highlights)
    }
}

/// The returned chunks are guaranteed to be single-line
pub fn annotate<'a, S, A>(
    lines: impl Iterator<Item = S> + 'a,
    annotations: impl IntoIterator<Item = (PointRange, A)> + 'a,
) -> impl Iterator<Item = (Line, Cow<'a, str>, Option<A>)> + 'a
where
    S: TextSlice<'a>,
    A: Copy,
{
    // Best effort slicing, will truncate any ranges that are out of bounds
    #[track_caller]
    fn slice<'a, S>(s: &S, byte_range: impl RangeBounds<usize> + fmt::Debug) -> Cow<'a, str>
    where
        S: TextSlice<'a>,
    {
        let start = match byte_range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };

        let end = match byte_range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => s.len_bytes(),
        };

        let end = end.min(s.len_bytes());
        let start = start.min(end);

        s.byte_slice(start..end).to_cow()
    }

    let mut annotations = annotations.into_iter().peekable();
    iter::from_coroutine(
        #[coroutine]
        move || {
            for (i, line) in lines.enumerate() {
                let line_len_bytes = line.len_bytes();

                let line_idx = Line::from(i);
                let mut j = 0;
                while let Some(&(range, annotation)) = annotations.peek() {
                    if range.start().line() > i {
                        break;
                    }

                    let start_col = if range.start().line() == i { range.start().col() } else { 0 };

                    if range.end().line() > i {
                        // If the highlight is a multi-line highlight,
                        // we style the entire line with that style and move on to highlight the next
                        // line (without next()ing the highlight iterator)
                        yield (line_idx, slice(&line, ..), Some(annotation));
                        // set `j` here so we don't try to highlight the same range again
                        j = line_len_bytes;
                        break;
                    }

                    let (range, annotation) = annotations.next().expect("just peeked");
                    let end_col =
                        if range.end().line() == i { range.end().col() } else { line_len_bytes };

                    if start_col < j {
                        // Sometimes annotations can overlap, we just arbitrarily use the first one of that range
                        continue;
                    }

                    if start_col > j {
                        yield (line_idx, slice(&line, j..start_col), None)
                    }

                    if end_col >= line_len_bytes {
                        // We're allowed to annotate places with no text, so the range end might be out of bounds
                        // In which case, we add another span with the remaining space.

                        // There's a bit of a bug here:
                        // If the line ends with a newline, then the padded span will be on the next line.
                        // The workaround is to return the line number as well, so the renderer can handle it.
                        yield (line_idx, slice(&line, start_col..), Some(annotation));
                        yield (
                            line_idx,
                            format!("{:width$}", "", width = end_col - line_len_bytes).into(),
                            Some(annotation),
                        )
                    } else {
                        yield (line_idx, slice(&line, start_col..end_col), Some(annotation));
                    }

                    j = end_col;
                }

                // Add in a span for the rest of the line that wasn't annotated
                if j < line_len_bytes {
                    yield (line_idx, slice(&line, j..), None);
                }

                // unconditionally yields a newline regardless of whether the line actually had one, I
                // don't think this causes any problems
                yield (line_idx, "\n".into(), None);
            }
        },
    )
    // fuse the iterator avoid panics due to misuse
    .fuse()
    .filter(|(_, text, _)| !text.is_empty())
}

impl<T: Text + ?Sized> Text for &T {
    type Slice<'a> = T::Slice<'a> where Self: 'a;

    #[inline]
    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        (**self).byte_slice(byte_range)
    }

    #[inline]
    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        (**self).line_slice(line_range)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        (**self).chars()
    }

    #[inline]
    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice<'_>> {
        (**self).lines()
    }

    #[inline]
    fn line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        (**self).line(line_idx)
    }

    #[inline]
    fn reader(&self) -> impl Read + '_ {
        (**self).reader()
    }
}

impl<T: TextBase + ?Sized> TextBase for &T {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }

    #[inline]
    fn len_lines(&self) -> usize {
        (**self).len_lines()
    }

    #[inline]
    fn byte_to_utf16_cu(&self, byte_idx: usize) -> usize {
        (**self).byte_to_utf16_cu(byte_idx)
    }

    #[inline]
    fn utf16_cu_to_byte(&self, cu_idx: usize) -> usize {
        (**self).utf16_cu_to_byte(cu_idx)
    }

    #[inline]
    fn len_bytes(&self) -> usize {
        (**self).len_bytes()
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
    fn get_char(&self, byte_idx: usize) -> Option<char> {
        (**self).get_char(byte_idx)
    }

    #[inline]
    fn try_line_to_byte(&self, line_idx: usize) -> Option<usize> {
        (**self).try_line_to_byte(line_idx)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        (**self).is_empty()
    }

    #[inline]
    fn byte_to_point(&self, byte_idx: usize) -> Point {
        (**self).byte_to_point(byte_idx)
    }

    #[inline]
    fn point_to_byte(&self, point: Point) -> usize {
        (**self).point_to_byte(point)
    }

    #[inline]
    fn point_range_to_byte_range(&self, range: PointRange) -> ops::Range<usize> {
        (**self).point_range_to_byte_range(range)
    }

    #[inline]
    fn point_or_byte_to_byte(&self, point_or_byte: PointOrByte) -> usize {
        (**self).point_or_byte_to_byte(point_or_byte)
    }

    #[inline]
    fn point_or_byte_to_point(&self, point_or_byte: PointOrByte) -> Point {
        (**self).point_or_byte_to_point(point_or_byte)
    }
}

impl<'a> TextSlice<'a> for Box<dyn AnyTextSlice<'a> + 'a> {
    type Slice = Self;

    type Chunks = Box<dyn DoubleEndedIterator<Item = &'a str> + 'a>;

    fn to_cow(&self) -> Cow<'a, str> {
        (**self).dyn_to_cow()
    }

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self {
        let range = (byte_range.start_bound().cloned(), byte_range.end_bound().cloned());
        Box::new((**self).dyn_byte_slice(range))
    }

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self {
        let range = (line_range.start_bound().cloned(), line_range.end_bound().cloned());
        Box::new((**self).dyn_line_slice(range))
    }

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + 'a {
        self.as_ref().dyn_chars()
    }

    fn lines(&self) -> impl DoubleEndedIterator<Item = Self> + 'a {
        self.as_ref().dyn_lines()
    }

    fn chunks(&self) -> Self::Chunks {
        self.as_ref().dyn_chunks()
    }

    fn line(&self, line_idx: usize) -> Option<Self> {
        self.as_ref().dyn_line(line_idx)
    }
}

struct TextReader<'a, I> {
    chunk: &'a [u8],
    chunks: I,
}

impl<'a, I> TextReader<'a, I> {
    fn new(chunks: I) -> Self {
        Self { chunk: &[], chunks }
    }
}

impl<'a, I> Read for TextReader<'a, I>
where
    I: Iterator<Item = &'a str>,
{
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.chunk.is_empty() {
            let Some(chunk) = self.chunks.next() else { return Ok(0) };
            self.chunk = chunk.as_bytes();
        }

        let n = buf.len().min(self.chunk.len());
        buf[..n].copy_from_slice(&self.chunk[..n]);
        self.chunk = &self.chunk[n..];
        Ok(n)
    }
}

#[cfg(test)]
mod tests;
