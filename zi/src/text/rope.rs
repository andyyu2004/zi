use ropey::RopeSlice;

use super::*;

impl<'a> TextSlice<'a> for crop::RopeSlice<'a> {
    fn to_cow(&self) -> Cow<'a, str> {
        let mut chunks = self.chunks();
        let fst = chunks.next().expect("RopeSlice is empty");
        match chunks.next() {
            Some(_) => Cow::Owned(self.to_string()),
            None => Cow::Borrowed(fst),
        }
    }

    fn slice(&self, byte_range: impl RangeBounds<usize>) -> Self {
        self.byte_slice(byte_range)
    }

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + 'a {
        self.chars()
    }

    fn lines(&self) -> impl Iterator<Item = Self> + 'a {
        self.lines()
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

    fn byte_slice<R: RangeBounds<usize>>(&self, byte_range: R) -> Self::Slice<'_> {
        self.slice(byte_range)
    }

    #[inline]
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
}
