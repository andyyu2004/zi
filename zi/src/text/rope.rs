use ropey::RopeSlice;

use super::*;

impl<'a> TextSlice<'a> for crop::RopeSlice<'a> {
    fn as_cow(&self) -> Cow<'a, str> {
        self.to_string().into()
    }
}

impl Text for crop::RopeSlice<'_> {
    type Slice<'a> = crop::RopeSlice<'a>
    where
        Self: 'a;

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + '_ {
        self.chars()
    }

    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>> + '_ {
        self.lines()
    }

    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        todo!()
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

impl TextMut for Rope {
    #[inline]
    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error> {
        let range = self.delta_to_byte_range(delta);
        let start = range.start;
        self.try_remove(range)?;
        self.try_insert(start, delta.text())
    }
}

impl Text for Rope {
    type Slice<'a> = RopeSlice<'a>;

    #[inline]
    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>> {
        self.lines()
    }

    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.get_line(line_idx)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        "".chars()
        // self.chars()
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

    #[inline]
    fn chunk_at_byte(&self, byte_idx: usize) -> &str {
        let (chunk, start_byte, _, _) = self.chunk_at_byte(byte_idx);
        &chunk[byte_idx - start_byte..]
    }
}
