use ropey::RopeSlice;

use super::*;

impl<'a> TextSlice<'a> for RopeSlice<'a> {
    #[inline]
    fn as_cow(&self) -> Cow<'a, str> {
        (*self).into()
    }
}

impl Text for RopeSlice<'_> {
    type Slice<'a> = RopeSlice<'a> where Self: 'a;

    #[inline]
    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>> {
        self.lines()
    }

    #[inline]
    fn lines_at(&self, line_idx: usize) -> impl Iterator<Item = Self::Slice<'_>> {
        self.lines_at(line_idx)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        "".chars()
        // self.chars()
    }

    #[inline]
    fn chars_at(&self, char_idx: usize) -> impl DoubleEndedIterator<Item = char> {
        "".chars()
        // self.chars_at(char_idx)
    }

    #[inline]
    fn chunks_in_byte_range(&self, range: ops::Range<usize>) -> impl Iterator<Item = &str> {
        self.byte_slice(range).chunks()
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
    fn line_to_char(&self, line_idx: usize) -> usize {
        self.line_to_char(line_idx)
    }

    #[inline]
    fn char_to_line(&self, char_idx: usize) -> usize {
        self.char_to_line(char_idx)
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
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.char_to_byte(char_idx)
    }

    #[inline]
    fn chunk_at_byte(&self, byte_idx: usize) -> &str {
        let (chunk, start_byte, _, _) = self.chunk_at_byte(byte_idx);
        &chunk[byte_idx - start_byte..]
    }
}
