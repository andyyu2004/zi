use regex_cursor::Cursor;

use crate::TextSlice;

// We need this since double-ended iterators are not what we need. We need a bidirectional
// iterator. Should consider implementing upstream but use this hack for now.
struct BidirectionalIterator<I: Iterator> {
    iter: I,
    idx: usize,
    items: Vec<I::Item>,
}

impl<I> Iterator for BidirectionalIterator<I>
where
    I: Iterator,
    I::Item: Copy,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.items.get(self.idx).copied() {
            self.idx += 1;
            return Some(item);
        }

        let item = self.iter.next()?;
        self.items.push(item);
        self.idx += 1;
        Some(item)
    }
}

impl<I> BidirectionalIterator<I>
where
    I: Iterator,
    I::Item: Copy,
{
    fn new(iter: I) -> Self {
        Self { iter, idx: 0, items: vec![] }
    }

    fn prev(&mut self) -> Option<I::Item> {
        if self.idx == 0 {
            return None;
        }

        self.idx -= 1;
        self.items.get(self.idx).copied()
    }
}

pub struct RopeCursor<'a, S: TextSlice<'a>> {
    chunks: BidirectionalIterator<S::Chunks>,
    current: &'a str,
    total_bytes: usize,
    offset: usize,
    at_chunk_end: bool,
}

impl<'a, S> RopeCursor<'a, S>
where
    S: TextSlice<'a>,
{
    pub fn new(slice: S) -> Self {
        let chunks = BidirectionalIterator::new(slice.chunks());
        let total_bytes = slice.len_bytes();
        let mut this = Self { total_bytes, chunks, offset: 0, current: "", at_chunk_end: true };
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
        self.current.as_bytes()
    }

    #[inline]
    fn utf8_aware(&self) -> bool {
        true
    }

    #[inline]
    fn advance(&mut self) -> bool {
        if !self.at_chunk_end {
            self.at_chunk_end = true;
            self.chunks.next();
        }

        for chunk in self.chunks.by_ref() {
            if chunk.is_empty() {
                continue;
            }
            self.offset += self.current.len();
            self.current = chunk;
            return true;
        }
        false
    }

    #[inline]
    fn backtrack(&mut self) -> bool {
        if self.at_chunk_end {
            self.at_chunk_end = false;
            self.chunks.prev();
        }

        while let Some(prev) = self.chunks.prev() {
            if prev.is_empty() {
                continue;
            }
            self.offset -= prev.len();
            self.current = prev;
            return true;
        }

        false
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

    #[test]
    fn text_cursor_smoke() {
        let rope = Rope::from("abc");
        let mut cursor = RopeCursor::new(rope.byte_slice(..));

        assert_eq!(cursor.total_bytes(), Some(3));
        assert_eq!(cursor.offset(), 0);
        assert_eq!(cursor.chunk(), b"a");

        assert!(cursor.advance());
        assert_eq!(cursor.offset(), 1);
        assert_eq!(cursor.chunk(), b"bc");

        assert!(!cursor.advance());
        assert_eq!(cursor.offset(), 1);
        assert_eq!(cursor.chunk(), b"bc");

        assert!(cursor.backtrack());
        assert_eq!(cursor.offset(), 0);
        assert_eq!(cursor.chunk(), b"a");

        assert!(!cursor.backtrack());

        assert!(cursor.advance());
        assert_eq!(cursor.offset(), 1);
        assert_eq!(cursor.chunk(), b"bc");

        assert!(cursor.backtrack());
        assert_eq!(cursor.offset(), 0);
        assert_eq!(cursor.chunk(), b"a");
    }

    proptest::proptest! {
        #[test]
        fn proptest_text_cursor_advance_and_backtrack(text in "[a-z]*") {
            let rope = Rope::from(text.as_str());
            let mut cursor = RopeCursor::new(rope.byte_slice(..));

            assert_eq!(cursor.total_bytes(), Some(text.len()));
            assert_eq!(cursor.offset(), 0);

            let mut chunks = vec![(
                cursor.offset(),
                cursor.chunk().to_owned(),
            )];
            while cursor.advance() {
                chunks.push((cursor.offset(), cursor.chunk().to_owned()));
            }

            chunks.pop();
            while cursor.backtrack() {
                let (offset, chunk) = chunks.pop().unwrap();
                assert_eq!(cursor.offset(), offset);
                assert_eq!(cursor.chunk(), chunk);
            }

            assert_eq!(cursor.offset(), 0);
            assert!(chunks.is_empty());
        }

        #[test]
        fn proptest_text_cursor_movement(s in ".*", actions in vec(any::<bool>(), 0..1000)) {
            let rope = Rope::from(s);
            let mut cursor = RopeCursor::new(rope.byte_slice(..));

            let mut i = 0;
            let mut chunks: Vec<*const u8> = vec![];

            for forward in actions {
                let offset = cursor.offset();
                let ptr = cursor.chunk().as_ptr();

                if i < chunks.len() {
                    assert_eq!(cursor.chunk().as_ptr(), chunks[i]);
                } else {
                    assert_eq!(i, chunks.len());
                    chunks.push(cursor.chunk().as_ptr());
                }

                let moved = if forward {
                    cursor.advance()
                } else {
                    cursor.backtrack()
                };

                if moved && forward {
                    i += 1;
                } else if moved && !forward {
                    i -= 1;
                }

                if moved {
                    assert_ne!(cursor.offset(), offset);
                    assert_ne!(cursor.chunk().as_ptr(), ptr);
                } else {
                    assert_eq!(cursor.offset(), offset);
                    assert_eq!(cursor.chunk().as_ptr(), ptr);
                }
            }
        }
    }
}
