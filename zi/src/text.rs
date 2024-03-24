mod cow_str_impl;
mod delta;
mod readonly;
mod rope;
mod rope_slice;
mod str_impl;

use std::borrow::Cow;
use std::ops::{Bound, RangeBounds};
use std::{fmt, iter, ops};

pub use self::delta::{Delta, DeltaRange};
pub use self::readonly::ReadonlyText;
use crate::{Line, Point, Range};

pub trait TextMut: Text {
    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error>;
}

pub trait AnyTextMut: AnyText {
    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error>;

    fn as_text(&self) -> &dyn AnyText;
}

impl<T: AnyText + TextMut> AnyTextMut for T {
    #[inline]
    fn as_text(&self) -> &dyn AnyText {
        self
    }

    #[inline]
    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error> {
        <T as TextMut>::edit(self, delta)
    }
}

/// dyn-safe interface for reading text
pub trait TextBase: fmt::Display {
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut>;

    fn len_lines(&self) -> usize;
    fn len_bytes(&self) -> usize;

    fn byte_to_line(&self, byte_idx: usize) -> usize;
    fn line_to_byte(&self, line_idx: usize) -> usize;

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
    fn delta_to_point_range(&self, delta: &Delta<'_>) -> Range {
        match delta.range() {
            DeltaRange::Point(p) => p,
            DeltaRange::Byte(range) => {
                Range::new(self.byte_to_point(range.start), self.byte_to_point(range.end))
            }
        }
    }

    #[inline]
    fn delta_to_byte_range(&self, delta: &Delta<'_>) -> ops::Range<usize> {
        match delta.range() {
            DeltaRange::Point(range) => self.point_range_to_byte_range(range),
            DeltaRange::Byte(range) => range,
        }
    }

    #[inline]
    fn point_to_byte(&self, point: Point) -> usize {
        self.line_to_byte(point.line().idx()) + point.col().idx()
    }

    #[inline]
    fn point_range_to_byte_range(&self, range: Range) -> ops::Range<usize> {
        self.point_to_byte(range.start())..self.point_to_byte(range.end())
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
    fn line_to_byte(&self, line_idx: usize) -> usize {
        (**self).line_to_byte(line_idx)
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
    fn dyn_lines(&self) -> Box<dyn Iterator<Item = Box<dyn AnyTextSlice<'a> + 'a>> + 'a>;
    fn dyn_chunks(&self) -> Box<dyn Iterator<Item = &'a str> + 'a>;
    fn dyn_get_line(&self, line_idx: usize) -> Option<Box<dyn AnyTextSlice<'a> + 'a>>;
}

pub trait AnyText: TextBase + fmt::Display {
    fn dyn_byte_slice(
        &self,
        byte_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'_> + '_>;

    fn dyn_line_slice(
        &self,
        line_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'_> + '_>;

    fn dyn_chars(&self) -> Box<dyn DoubleEndedIterator<Item = char> + '_>;
    fn dyn_lines(&self) -> Box<dyn Iterator<Item = Box<dyn AnyTextSlice<'_> + '_>> + '_>;
    fn dyn_get_line(&self, line_idx: usize) -> Option<Box<dyn AnyTextSlice<'_> + '_>>;
}

impl<'a> TextSlice<'a> for &'a dyn AnyTextSlice<'a> {
    type Slice = Box<dyn AnyTextSlice<'a> + 'a>;

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

    fn lines(&self) -> impl Iterator<Item = Self::Slice> + 'a {
        self.dyn_lines()
    }

    fn chunks(&self) -> impl Iterator<Item = &'a str> + 'a {
        self.dyn_chunks()
    }

    fn get_line(&self, line_idx: usize) -> Option<Self::Slice> {
        self.dyn_get_line(line_idx)
    }
}

impl Text for dyn AnyText + '_ {
    type Slice<'a> = Box<dyn AnyTextSlice<'a> + 'a>
    where
        Self: 'a;

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        self.dyn_line_slice((line_range.start_bound().cloned(), line_range.end_bound().cloned()))
    }

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        self.dyn_byte_slice((byte_range.start_bound().cloned(), byte_range.end_bound().cloned()))
    }

    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.dyn_get_line(line_idx)
    }

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        self.dyn_chars()
    }

    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>> {
        self.dyn_lines()
    }
}

impl<T: Text + ?Sized> AnyText for T {
    fn dyn_byte_slice(
        &self,
        byte_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'_> + '_> {
        Box::new(self.byte_slice(byte_range))
    }

    fn dyn_line_slice(
        &self,
        line_range: (Bound<usize>, Bound<usize>),
    ) -> Box<dyn AnyTextSlice<'_> + '_> {
        Box::new(self.line_slice(line_range))
    }

    fn dyn_chars(&self) -> Box<dyn DoubleEndedIterator<Item = char> + '_> {
        Box::new(self.chars())
    }

    fn dyn_lines(&self) -> Box<dyn Iterator<Item = Box<dyn AnyTextSlice<'_> + '_>> + '_> {
        Box::new(self.lines().map(|s| Box::new(s) as _))
    }

    fn dyn_get_line(&self, line_idx: usize) -> Option<Box<dyn AnyTextSlice<'_> + '_>> {
        self.get_line(line_idx).map(|s| Box::new(s) as _)
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

    fn dyn_lines(&self) -> Box<dyn Iterator<Item = Box<dyn AnyTextSlice<'a> + 'a>> + 'a> {
        Box::new(<T as TextSlice<'a>>::lines(self).map(|s| Box::new(s) as _))
    }

    fn dyn_chunks(&self) -> Box<dyn Iterator<Item = &'a str> + 'a> {
        Box::new(<T as TextSlice<'a>>::chunks(self))
    }

    fn dyn_get_line(&self, line_idx: usize) -> Option<Box<dyn AnyTextSlice<'a> + 'a>> {
        <T as TextSlice<'a>>::get_line(self, line_idx).map(move |s| Box::new(s) as _)
    }
}

/// Similar to [`Text`] except the returned lifetimes are tied to `'a` instead of `'self`.
pub trait TextSlice<'a>: TextBase + Sized {
    type Slice: TextSlice<'a>;

    fn to_cow(&self) -> Cow<'a, str>;

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice;

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice;

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + 'a;

    fn lines(&self) -> impl Iterator<Item = Self::Slice> + 'a;

    fn chunks(&self) -> impl Iterator<Item = &'a str> + 'a;

    fn get_line(&self, line_idx: usize) -> Option<Self::Slice>;
}

pub trait Text: TextBase {
    type Slice<'a>: TextSlice<'a>
    where
        Self: 'a;

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice<'_>;

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice<'_>;

    fn chars(&self) -> impl DoubleEndedIterator<Item = char>;

    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>>;

    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>>;

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

/// The returned chunks are guaranteed to be single-line
pub fn annotate<'a, S, A>(
    lines: impl Iterator<Item = S> + 'a,
    annotations: impl IntoIterator<Item = (Range, A)> + 'a,
) -> impl Iterator<Item = (Line, Cow<'a, str>, Option<A>)> + 'a
where
    S: TextSlice<'a>,
    A: Copy,
{
    fn slice<'a, S>(s: &S, byte_range: impl RangeBounds<usize>) -> Cow<'a, str>
    where
        S: TextSlice<'a>,
    {
        s.byte_slice(byte_range).to_cow()
    }

    let mut annotations = annotations.into_iter().peekable();
    iter::from_coroutine(move || {
        for (i, line) in lines.enumerate() {
            let line_len_bytes = line.len_bytes();

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
                    yield (line_idx, slice(&line, ..), Some(annotation));
                    // set `j` here so we don't try to highlight the same range again
                    j = line_len_bytes;
                    break;
                }

                let (range, annotation) = annotations.next().expect("just peeked");
                let end_col = if range.end().line().idx() == i {
                    range.end().col().idx()
                } else {
                    line_len_bytes
                };

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
        }
    })
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
    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>> {
        (**self).lines()
    }

    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        (**self).get_line(line_idx)
    }
}

impl<T: TextBase + ?Sized> TextBase for &T {
    #[inline]
    fn len_bytes(&self) -> usize {
        (**self).len_bytes()
    }

    #[inline]
    fn len_lines(&self) -> usize {
        (**self).len_lines()
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
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }
}

impl<'a> TextSlice<'a> for Box<dyn AnyTextSlice<'a> + 'a> {
    type Slice = Self;

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

    fn lines(&self) -> impl Iterator<Item = Self> + 'a {
        self.as_ref().dyn_lines()
    }

    fn chunks(&self) -> impl Iterator<Item = &'a str> + 'a {
        self.as_ref().dyn_chunks()
    }

    fn get_line(&self, line_idx: usize) -> Option<Self> {
        self.as_ref().dyn_get_line(line_idx)
    }
}

#[cfg(test)]
mod tests;
