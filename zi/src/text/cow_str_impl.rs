use super::*;

impl<'a> TextSlice<'a> for Cow<'a, str> {
    fn as_cow(&self) -> Cow<'a, str> {
        self.clone()
    }
}

impl Text for Cow<'_, str> {
    type Slice<'a> = Cow<'a, str> where Self: 'a;

    #[inline]
    fn lines(&self) -> impl Iterator<Item = Cow<'_, str>> {
        <str as Text>::lines(self.as_ref())
    }

    #[inline]
    fn lines_at(&self, line_idx: usize) -> impl Iterator<Item = Cow<'_, str>> {
        self.as_ref().lines_at(line_idx)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        <str as Text>::chars(self.as_ref())
    }

    #[inline]
    fn chars_at(&self, char_idx: usize) -> impl DoubleEndedIterator<Item = char> {
        self.as_ref().chars_at(char_idx)
    }

    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.as_ref().get_line(line_idx)
    }

    #[inline]
    fn chunks_in_byte_range(&self, range: std::ops::Range<usize>) -> impl Iterator<Item = &str> {
        self.as_ref().chunks_in_byte_range(range)
    }
}

impl TextBase for Cow<'_, str> {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }

    #[inline]
    fn len_lines(&self) -> usize {
        self.as_ref().len_lines()
    }

    #[inline]
    fn len_bytes(&self) -> usize {
        self.as_ref().len_bytes()
    }

    #[inline]
    fn line_to_char(&self, line_idx: usize) -> usize {
        self.as_ref().line_to_char(line_idx)
    }

    #[inline]
    fn char_to_line(&self, char_idx: usize) -> usize {
        self.as_ref().char_to_line(char_idx)
    }

    #[inline]
    fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.as_ref().byte_to_line(byte_idx)
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.as_ref().line_to_byte(line_idx)
    }

    #[inline]
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.as_ref().char_to_byte(char_idx)
    }

    #[inline]
    fn chunk_at_byte(&self, byte_idx: usize) -> &str {
        self.as_ref().chunk_at_byte(byte_idx)
    }
}
