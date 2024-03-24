use ropey::RopeSlice;

use super::*;

impl<'a> TextSlice<'a> for crop::RopeSlice<'a> {
    type Slice = Self;

    fn to_cow(&self) -> Cow<'a, str> {
        let mut chunks = self.chunks();
        let fst = chunks.next().expect("RopeSlice is empty");
        match chunks.next() {
            Some(_) => Cow::Owned(self.to_string()),
            None => Cow::Borrowed(fst),
        }
    }

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self {
        (*self).byte_slice(byte_range)
    }

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice {
        (*self).line_slice(line_range)
    }

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + 'a {
        (*self).chars()
    }

    fn lines(&self) -> impl Iterator<Item = Self> + 'a {
        (*self).lines()
    }

    fn chunks(&self) -> impl Iterator<Item = &'a str> + 'a {
        (*self).chunks()
    }

    fn get_line(&self, line_idx: usize) -> Option<Self> {
        if line_idx < self.len_lines() { Some(self.line(line_idx)) } else { None }
    }
}

impl TextBase for crop::RopeSlice<'_> {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }

    #[inline]
    fn len_lines(&self) -> usize {
        self.line_len()
    }

    #[inline]
    fn len_bytes(&self) -> usize {
        self.byte_len()
    }

    #[inline]
    fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.line_of_byte(byte_idx)
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.byte_of_line(line_idx)
    }
}
