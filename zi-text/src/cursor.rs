use regex_cursor::Cursor;

pub struct RopeCursor<'a> {
    chunks: crop::iter::Chunks<'a>,
    chunk: &'a str,
    total_bytes: usize,
    offset: usize,
}

impl<'a> RopeCursor<'a> {
    pub fn new(slice: crop::RopeSlice<'a>) -> Self {
        let mut this =
            Self { total_bytes: slice.byte_len(), offset: 0, chunks: slice.chunks(), chunk: "" };
        this.advance();
        this
    }
}

impl<'a> From<crop::RopeSlice<'a>> for RopeCursor<'a> {
    fn from(slice: crop::RopeSlice<'a>) -> Self {
        Self::new(slice)
    }
}

impl<'a> From<&'a crop::Rope> for RopeCursor<'a> {
    fn from(rope: &'a crop::Rope) -> Self {
        Self::new(rope.byte_slice(..))
    }
}

impl<'a> Cursor for RopeCursor<'a> {
    #[inline]
    fn chunk(&self) -> &[u8] {
        self.chunk.as_bytes()
    }

    #[inline]
    fn advance(&mut self) -> bool {
        let Some(chunk) = self.chunks.next() else { return false };
        assert!(!chunk.is_empty());
        self.chunk = chunk;
        self.offset += chunk.len();
        true
    }

    #[inline]
    fn backtrack(&mut self) -> bool {
        let Some(chunk) = self.chunks.next_back() else { return false };
        assert!(!chunk.is_empty());
        self.chunk = chunk;
        self.offset -= chunk.len();
        true
    }

    #[inline]
    fn total_bytes(&self) -> Option<usize> {
        Some(self.total_bytes)
    }

    #[inline]
    fn offset(&self) -> usize {
        self.offset
    }
}
