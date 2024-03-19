use super::*;

impl TextMut for Rope {
    #[inline]
    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error> {
        let range = self.delta_to_char_range(delta);
        let start = range.start;
        self.try_remove(range)?;
        self.try_insert(start, delta.text())
    }
}

impl Text for Rope {
    #[inline]
    fn lines_at(&self, line_idx: usize) -> impl Iterator<Item = Cow<'_, str>> {
        self.lines_at(line_idx).map(Into::into)
    }

    #[inline]
    fn chars_at(&self, char_idx: usize) -> impl BidirectionalIterator<Item = char> {
        self.chars_at(char_idx)
    }

    #[inline]
    fn chunks_in_byte_range(&self, range: ops::Range<usize>) -> impl Iterator<Item = &str> {
        self.byte_slice(range).chunks()
    }
}

impl TextBase for Rope {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        Some(self)
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
    fn len_bytes(&self) -> usize {
        self.len_bytes()
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
