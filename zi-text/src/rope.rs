use super::*;

impl TextMut for crop::Rope {
    #[inline]
    fn edit(&mut self, deltas: &Deltas<'_>) -> Deltas<'static> {
        deltas.apply(self)
    }
}

impl Text for crop::Rope {
    type Slice<'a> = crop::RopeSlice<'a>;

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        self.byte_slice(byte_range)
    }

    #[inline]
    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        self.line_slice(line_range)
    }

    #[inline]
    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice<'_>> {
        self.lines()
    }

    #[inline]
    fn line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        if line_idx < self.line_len() { Some(self.line(line_idx)) } else { None }
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        self.chars()
    }

    #[inline]
    fn reader(&self) -> impl Read + '_ {
        TextReader::new(self.chunks())
    }
}

impl TextBase for crop::Rope {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        Some(self)
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
    fn len_utf16_cu(&self) -> usize {
        self.utf16_len()
    }

    #[inline]
    fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.line_of_byte(byte_idx)
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.byte_of_line(line_idx)
    }

    #[inline]
    fn get_char(&self, byte_idx: usize) -> Option<char> {
        (*self).byte_slice(byte_idx..).chars().next()
    }

    #[inline]
    fn utf16_cu_to_byte(&self, cu_idx: usize) -> usize {
        self.byte_of_utf16_code_unit(cu_idx)
    }

    #[inline]
    fn byte_to_utf16_cu(&self, byte_idx: usize) -> usize {
        self.utf16_code_unit_of_byte(byte_idx)
    }
}

impl<'a> TextSlice<'a> for crop::RopeSlice<'a> {
    type Slice = Self;
    type Chunks = crop::iter::Chunks<'a>;

    fn to_cow(&self) -> Cow<'a, str> {
        let mut chunks = self.chunks();
        let fst = chunks.next().unwrap_or("");
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

    fn lines(&self) -> impl DoubleEndedIterator<Item = Self> + 'a {
        (*self).lines()
    }

    fn chunks(&self) -> Self::Chunks {
        (*self).chunks()
    }

    fn line(&self, line_idx: usize) -> Option<Self> {
        if line_idx < self.line_len() { Some((*self).line(line_idx)) } else { None }
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
    fn len_utf16_cu(&self) -> usize {
        self.utf16_len()
    }

    #[inline]
    fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.line_of_byte(byte_idx)
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.byte_of_line(line_idx)
    }

    #[inline]
    fn get_char(&self, byte_idx: usize) -> Option<char> {
        (*self).byte_slice(byte_idx..).chars().next()
    }

    #[inline]
    fn utf16_cu_to_byte(&self, cu_idx: usize) -> usize {
        self.byte_of_utf16_code_unit(cu_idx)
    }

    #[inline]
    fn byte_to_utf16_cu(&self, byte_idx: usize) -> usize {
        self.utf16_code_unit_of_byte(byte_idx)
    }
}
