mod cow_str_impl;
mod delta;
mod readonly;
mod rope;
mod rope_slice;
mod str_impl;

use std::borrow::Cow;
use std::ops::RangeBounds;
use std::slice::SliceIndex;
use std::{fmt, iter, ops};

use ropey::Rope;

pub use self::delta::{Delta, DeltaRange};
pub use self::readonly::ReadonlyText;
use crate::{Line, Point, Range};

pub trait TextMut: Text {
    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error>;
}

pub trait AnyTextMut: AnyText {
    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error>;
}

impl<T: AnyText + TextMut> AnyTextMut for T {
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

    fn chunk_at_byte(&self, byte_idx: usize) -> &str;

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

pub trait AnyTextSlice<'a>: AnyText {
    fn into_cow(self: Box<Self>) -> Cow<'a, str>;
}

impl<'a, T: TextSlice<'a>> AnyTextSlice<'a> for T {
    fn into_cow(self: Box<Self>) -> Cow<'a, str> {
        (*self).into()
    }
}

pub trait AnyText: TextBase + fmt::Display {
    fn dyn_lines_at(
        &self,
        line_idx: usize,
    ) -> Box<dyn Iterator<Item = Box<dyn AnyTextSlice<'_> + '_>> + '_>;
    fn dyn_chars_at(&self, char_idx: usize) -> Box<dyn DoubleEndedIterator<Item = char> + '_>;

    fn dyn_chars(&self) -> Box<dyn DoubleEndedIterator<Item = char> + '_>;
    fn dyn_lines(&self) -> Box<dyn Iterator<Item = Box<dyn AnyTextSlice<'_> + '_>> + '_>;

    fn dyn_get_line(&self, line_idx: usize) -> Option<Box<dyn AnyTextSlice<'_> + '_>>;

    fn dyn_chunks_in_byte_range(
        &self,
        range: ops::Range<usize>,
    ) -> Box<dyn Iterator<Item = &str> + '_>;
}

impl<T: Text> AnyText for T {
    fn dyn_lines_at(
        &self,
        line_idx: usize,
    ) -> Box<dyn Iterator<Item = Box<dyn AnyTextSlice<'_> + '_>> + '_> {
        Box::new(<T as Text>::lines_at(self, line_idx).map(|s| Box::new(s) as _))
    }

    fn dyn_chars(&self) -> Box<dyn DoubleEndedIterator<Item = char> + '_> {
        Box::new(<T as Text>::chars(self))
    }

    fn dyn_chars_at(&self, char_idx: usize) -> Box<dyn DoubleEndedIterator<Item = char> + '_> {
        Box::new(<T as Text>::chars_at(self, char_idx))
    }

    fn dyn_lines(&self) -> Box<dyn Iterator<Item = Box<dyn AnyTextSlice<'_> + '_>> + '_> {
        Box::new(<T as Text>::lines(self).map(|s| Box::new(s) as _))
    }

    fn dyn_get_line(&self, line_idx: usize) -> Option<Box<dyn AnyTextSlice<'_> + '_>> {
        <T as Text>::get_line(self, line_idx).map(move |s| Box::new(s) as _)
    }

    fn dyn_chunks_in_byte_range(
        &self,
        range: ops::Range<usize>,
    ) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(<T as Text>::chunks_in_byte_range(self, range))
    }
}

pub trait TextSlice<'a>: Text + Into<Cow<'a, str>> {
    fn as_cow(&self) -> Cow<'a, str>;
}

pub trait Text: TextBase {
    type Slice<'a>: TextSlice<'a>
    where
        Self: 'a;

    fn lines_at(&self, line_idx: usize) -> impl Iterator<Item = Self::Slice<'_>>;
    fn chars_at(&self, char_idx: usize) -> impl DoubleEndedIterator<Item = char>;

    fn chunks_in_byte_range(&self, byte_range: ops::Range<usize>) -> impl Iterator<Item = &str>;

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + '_;

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
    // A specialized slice that preserves the borrow if possible
    fn slice<'a, S>(s: &S, bounds: impl SliceIndex<str, Output = str>) -> Cow<'a, str>
    where
        S: TextSlice<'a>,
    {
        match s.as_cow() {
            Cow::Borrowed(s) => Cow::Borrowed(&s[bounds]),
            Cow::Owned(s) => Cow::Owned(s[bounds].to_owned()),
        }
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
                    yield (line_idx, line.as_cow(), Some(annotation));
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
    fn lines_at(&self, line_idx: usize) -> impl Iterator<Item = Self::Slice<'_>> {
        (**self).lines_at(line_idx)
    }

    #[inline]
    fn chars_at(&self, char_idx: usize) -> impl DoubleEndedIterator<Item = char> {
        (**self).chars_at(char_idx)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        (**self).chars()
    }

    #[inline]
    fn chunks_in_byte_range(&self, range: ops::Range<usize>) -> impl Iterator<Item = &str> {
        (**self).chunks_in_byte_range(range)
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
    fn chunk_at_byte(&self, byte_idx: usize) -> &str {
        (**self).chunk_at_byte(byte_idx)
    }

    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }
}

impl Text for dyn AnyText + '_ {
    type Slice<'a> = Cow<'a, str>
    where
        Self: 'a;

    #[inline]
    fn lines_at(&self, line_idx: usize) -> impl Iterator<Item = Self::Slice<'_>> {
        self.dyn_lines_at(line_idx).map(|s| s.into_cow())
    }

    fn chars_at(&self, char_idx: usize) -> impl DoubleEndedIterator<Item = char> {
        self.dyn_chars_at(char_idx)
    }

    fn chunks_in_byte_range(&self, byte_range: ops::Range<usize>) -> impl Iterator<Item = &str> {
        self.dyn_chunks_in_byte_range(byte_range)
    }

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + '_ {
        self.dyn_chars()
    }

    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>> {
        self.dyn_lines().map(|s| s.into_cow())
    }

    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.dyn_get_line(line_idx).map(|s| s.into_cow())
    }
}

#[cfg(test)]
mod tests;
