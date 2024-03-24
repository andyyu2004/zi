use super::*;

/// Some magic to match the behaviour of `[ropey::Ropey]`
fn str_lines(s: &str) -> impl Iterator<Item = Cow<'_, str>> {
    // append an empty line if the string ends with a newline or is empty (to match ropey's behaviour)
    s.split_inclusive('\n').chain((s.is_empty() || s.ends_with('\n')).then_some("")).map(Into::into)
}

impl<'a> TextSlice<'a> for &'a str {
    #[inline]
    fn as_cow(&self) -> Cow<'a, str> {
        Cow::Borrowed(*self)
    }
}

impl Text for str {
    type Slice<'a> = Cow<'a, str>;

    #[inline]
    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>> {
        str_lines(self)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        self.chars()
    }

    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        str_lines(self).nth(line_idx)
    }
}

/// Naive implementation of [`LazyText`] for `str`.
/// Most of these methods are O(n) and large strs should be avoided.
impl TextBase for str {
    #[inline]
    fn len_bytes(&self) -> usize {
        self.len()
    }

    #[inline]
    fn len_lines(&self) -> usize {
        1 + str_lines(self).filter(|line| line.ends_with('\n')).count()
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        str_lines(self).take(line_idx).map(|l| l.len()).sum()
    }

    fn byte_to_line(&self, mut byte_idx: usize) -> usize {
        assert!(byte_idx <= self.len(), "byte_idx out of bounds: {byte_idx}");
        str_lines(self)
            .take_while(|l| {
                if l.len() > byte_idx {
                    return false;
                }
                byte_idx -= l.len();
                true
            })
            .count()
    }

    #[inline]
    fn chunk_at_byte(&self, byte_idx: usize) -> &str {
        &self[byte_idx..]
    }

    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }
}
