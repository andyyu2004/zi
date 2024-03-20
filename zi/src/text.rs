mod delta;
mod readonly;
mod rope;
mod str_impl;

use std::borrow::Cow;
use std::ops::RangeBounds;
use std::{fmt, iter, ops};

use ropey::Rope;
use stdx::iter::BidirectionalIterator;

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
    fn len_chars(&self) -> usize;
    fn len_bytes(&self) -> usize;

    fn get_line(&self, line_idx: usize) -> Option<Cow<'_, str>>;
    fn get_char(&self, char_idx: usize) -> Option<char>;

    fn line_to_char(&self, line_idx: usize) -> usize;
    fn char_to_line(&self, char_idx: usize) -> usize;

    fn byte_to_line(&self, byte_idx: usize) -> usize;
    fn line_to_byte(&self, line_idx: usize) -> usize;

    fn char_to_byte(&self, char_idx: usize) -> usize;

    fn chunk_at_byte(&self, byte_idx: usize) -> &str;

    #[inline]
    fn line(&self, line: usize) -> Cow<'_, str> {
        self.get_line(line).unwrap_or_else(|| {
            panic!("line out of bounds: {line}");
        })
    }

    #[inline]
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
}

pub trait AnyText: TextBase + fmt::Display {
    fn lines_at(&self, line_idx: usize) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_>;
    fn chars_at(&self, char_idx: usize) -> Box<dyn BidirectionalIterator<Item = char> + '_>;

    fn lines(&self) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_>;

    fn chunks_in_byte_range(&self, range: ops::Range<usize>)
    -> Box<dyn Iterator<Item = &str> + '_>;
}

impl<T: Text> AnyText for T {
    fn lines_at(&self, line_idx: usize) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        Box::new(<T as Text>::lines_at(self, line_idx))
    }

    fn chars_at(&self, char_idx: usize) -> Box<dyn BidirectionalIterator<Item = char> + '_> {
        Box::new(<T as Text>::chars_at(self, char_idx))
    }

    fn lines(&self) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        Box::new(<T as Text>::lines(self))
    }

    fn chunks_in_byte_range(
        &self,
        range: ops::Range<usize>,
    ) -> Box<dyn Iterator<Item = &str> + '_> {
        Box::new(<T as Text>::chunks_in_byte_range(self, range))
    }
}

pub trait Text: TextBase {
    fn lines_at(&self, line_idx: usize) -> impl Iterator<Item = Cow<'_, str>>;
    fn chars_at(&self, char_idx: usize) -> impl BidirectionalIterator<Item = char>;

    fn chunks_in_byte_range(&self, byte_range: ops::Range<usize>) -> impl Iterator<Item = &str>;

    #[inline]
    fn lines(&self) -> impl Iterator<Item = Cow<'_, str>> {
        self.lines_at(0)
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

/// The returned chunks are guaranteed to be single-line
pub fn annotate<'a, T: Copy>(
    lines: impl Iterator<Item = Cow<'a, str>> + 'a,
    annotations: impl IntoIterator<Item = (Range, T)> + 'a,
) -> impl Iterator<Item = (Line, Cow<'a, str>, Option<T>)> + 'a {
    // A specialized slice that preserves the borrow if possible
    fn slice_cow<'a, R: RangeBounds<usize>>(s: &Cow<'a, str>, bounds: R) -> Cow<'a, str> {
        let start = bounds.start_bound().map(|&c| s.char_to_byte(c));
        let end = bounds.end_bound().map(|&c| s.char_to_byte(c));
        match s {
            Cow::Borrowed(s) => Cow::Borrowed(&s[(start, end)]),
            Cow::Owned(s) => Cow::Owned(s[(start, end)].to_owned()),
        }
    }

    let mut annotations = annotations.into_iter().peekable();
    iter::from_coroutine(move || {
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

impl<T: Text + ?Sized> Text for &T {
    #[inline]
    fn lines_at(&self, line_idx: usize) -> impl Iterator<Item = Cow<'_, str>> {
        (**self).lines_at(line_idx)
    }

    #[inline]
    fn chars_at(&self, char_idx: usize) -> impl BidirectionalIterator<Item = char> {
        (**self).chars_at(char_idx)
    }

    #[inline]
    fn chunks_in_byte_range(&self, range: ops::Range<usize>) -> impl Iterator<Item = &str> {
        (**self).chunks_in_byte_range(range)
    }

    #[inline]
    fn lines(&self) -> impl Iterator<Item = Cow<'_, str>> {
        (**self).lines()
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
    fn len_chars(&self) -> usize {
        (**self).len_chars()
    }

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
    fn chunk_at_byte(&self, byte_idx: usize) -> &str {
        (**self).chunk_at_byte(byte_idx)
    }

    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }

    #[inline]
    fn line(&self, line: usize) -> Cow<'_, str> {
        (**self).line(line)
    }
}

#[cfg(test)]
mod tests;
