use std::cmp;
use std::sync::OnceLock;

use stdx::iter::IteratorExt;

use super::*;

fn empty_slice<'a>() -> crop::RopeSlice<'a> {
    static EMPTY_ROPE: OnceLock<crop::Rope> = OnceLock::new();
    EMPTY_ROPE.get_or_init(crop::Rope::new).byte_slice(..)
}

impl TextMut for crop::Rope {
    #[inline]
    fn edit(&mut self, delta: &Delta<'_>) {
        let range = self.delta_to_byte_range(delta);
        self.replace(range, delta.text());
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
        self.lines().default_if_empty(empty_slice())
    }

    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        // NOTE: we're using the ropes `line_len` not the adjusted `len_lines`
        let n = self.line_len();
        match line_idx.cmp(&n) {
            cmp::Ordering::Less => Some(self.line(line_idx)),
            cmp::Ordering::Equal if line_idx == 0 => Some(empty_slice()),
            _ => None,
        }
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        self.chars()
    }
}

impl TextBase for crop::Rope {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        Some(self)
    }

    #[inline]
    fn len_lines(&self) -> usize {
        self.line_len().max(1)
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

impl<'a> TextSlice<'a> for crop::RopeSlice<'a> {
    type Slice = Self;

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

    fn lines(&self) -> impl Iterator<Item = Self> + 'a {
        (*self).lines().default_if_empty(empty_slice())
    }

    fn chunks(&self) -> impl Iterator<Item = &'a str> + 'a {
        (*self).chunks()
    }

    fn get_line(&self, line_idx: usize) -> Option<Self> {
        // NOTE: we're using the ropes `line_len` not the adjusted `len_lines`
        let n = self.line_len();
        match line_idx.cmp(&n) {
            cmp::Ordering::Less => Some(self.line(line_idx)),
            cmp::Ordering::Equal if line_idx == 0 => Some(empty_slice()),
            _ => None,
        }
    }
}

impl TextBase for crop::RopeSlice<'_> {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }

    #[inline]
    fn len_lines(&self) -> usize {
        self.line_len().max(1)
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
