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
    fn utf8_aware(&self) -> bool {
        true
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
        self.offset.checked_sub(self.chunk.len()).unwrap();
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
    use proptest::arbitrary::any;
    use proptest::collection::vec;
    use regex_cursor::engines::meta::Regex;
    use regex_cursor::{Cursor, Input};

    use crate::RopeCursor;

    #[test]
    fn text_cursor_search() {
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

    proptest::proptest! {
        #[test]
        fn proptest_text_cursor_api(text in ".*") {
            let rope = Rope::from(text.as_str());
            let mut cursor = RopeCursor::new(rope.byte_slice(..));

            let mut k = 0;
            while k < text.len() {
                assert_eq!(cursor.total_bytes(), Some(text.len()));
                assert_eq!(k, cursor.offset());
                assert_eq!(cursor.chunk(), text[k..k + cursor.chunk().len()].as_bytes());

                k += cursor.chunk().len();

                if k == text.len() {
                    assert!(!cursor.advance());
                    break;
                }

                assert!(cursor.advance());
            }

            assert!(!cursor.advance());
        }

        #[test]
        fn proptest_text_cursor(s in ".*", actions in vec(any::<bool>(), 0..1000)) {
            let rope = Rope::from(s);
            let mut cursor = RopeCursor::new(rope.byte_slice(..));
            for forward in actions {
                let offset = cursor.offset();
                let ptr = cursor.chunk().as_ptr();

                let moved = if forward {
                    cursor.advance()
                } else {
                    cursor.backtrack()
                };

                if moved {
                    assert_eq!(cursor.offset(), offset);
                    assert_eq!(cursor.chunk().as_ptr(), ptr);
                } else {
                    assert_ne!(cursor.offset(), offset);
                    assert_ne!(cursor.chunk().as_ptr(), ptr);
                }
            }
        }
    }
}
