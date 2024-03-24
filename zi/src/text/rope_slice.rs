use ropey::RopeSlice;

use super::*;

impl<'a> TextSlice<'a> for RopeSlice<'a> {
    fn to_cow(&self) -> Cow<'a, str> {
        (*self).into()
    }

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self {
        self.byte_slice(byte_range)
    }

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + 'a {
        "".chars()
    }

    fn lines(&self) -> impl Iterator<Item = Self> + 'a {
        self.lines()
    }

    fn get_line(&self, line_idx: usize) -> Option<Self> {
        self.get_line(line_idx)
    }

    type Slice = Self;

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice {
        // TODO remove impl
        self.byte_slice(..)
    }

    fn chunks(&self) -> impl Iterator<Item = &'a str> + 'a {
        self.chunks()
    }
}

impl Text for RopeSlice<'_> {
    type Slice<'a> = RopeSlice<'a> where Self: 'a;

    fn byte_slice<R: RangeBounds<usize>>(&self, byte_range: R) -> Self::Slice<'_> {
        self.slice(byte_range)
    }

    fn line_slice<R>(&self, line_range: R) -> Self::Slice<'_>
    where
        R: RangeBounds<usize>,
    {
        // TODO will remove this
        self.byte_slice(line_range)
    }

    #[inline]
    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>> {
        self.lines()
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        "".chars()
        // self.chars()
    }

    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.get_line(line_idx)
    }
}

impl TextBase for RopeSlice<'_> {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }

    #[inline]
    fn len_lines(&self) -> usize {
        self.len_lines()
    }

    #[inline]
    fn len_bytes(&self) -> usize {
        self.len_bytes()
    }

    #[inline]
    fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.byte_to_line(byte_idx)
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.line_to_byte(line_idx)
    }
}
