use regex_cursor::Cursor;

use crate::TextSlice;

pub struct RopeCursor<'a, S: TextSlice<'a>> {
    chunks: S::Chunks,
    chunk: &'a str,
    total_bytes: usize,
    offset: usize,
}

impl<'a, S> RopeCursor<'a, S>
where
    S: TextSlice<'a>,
{
    pub fn new(slice: S) -> Self {
        let chunks = slice.chunks();
        let total_bytes = slice.len_bytes();
        let mut this = Self { total_bytes, chunks, offset: 0, chunk: "" };
        this.advance();
        this
    }
}

impl<'a> From<&'a crop::Rope> for RopeCursor<'a, crop::RopeSlice<'a>> {
    fn from(rope: &'a crop::Rope) -> Self {
        Self::new(rope.byte_slice(..))
    }
}

impl<'a, S> Cursor for RopeCursor<'a, S>
where
    S: TextSlice<'a>,
{
    #[inline]
    fn chunk(&self) -> &[u8] {
        self.chunk.as_bytes()
    }

    #[inline]
    fn advance(&mut self) -> bool {
        let Some(chunk) = self.chunks.next() else { return false };
        assert!(!chunk.is_empty());
        self.offset += self.chunk.len();
        self.chunk = chunk;
        true
    }

    #[inline]
    fn backtrack(&mut self) -> bool {
        let Some(chunk) = self.chunks.next_back() else { return false };
        assert!(!chunk.is_empty());
        self.offset -= self.chunk.len();
        self.chunk = chunk;
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

#[cfg(test)]
mod tests {
    use crop::Rope;
    use regex_cursor::engines::meta::Regex;
    use regex_cursor::Input;

    use crate::RopeCursor;

    #[test]
    fn text_cursor() {
        const N: usize = 500;

        for k in 0..N {
            let regex = Regex::new("abc").unwrap();
            let rope = Rope::from("abc".repeat(k));
            let input = RopeCursor::new(rope.byte_slice(..));
            let n = regex
                .find_iter(Input::new(input))
                .enumerate()
                .inspect(|(i, mat)| {
                    assert_eq!(mat.range(), (i * 3..i * 3 + 3));
                })
                .count();

            assert_eq!(k, n, "k = {k}");
        }
    }
}
