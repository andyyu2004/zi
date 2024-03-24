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
    fn lines_at(&self, line_idx: usize) -> impl Iterator<Item = Self::Slice<'_>> {
        str_lines(self).skip(line_idx)
    }

    #[inline]
    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>> {
        str_lines(self)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        self.chars_at(0)
    }

    #[inline]
    fn chars_at(&self, char_idx: usize) -> impl DoubleEndedIterator<Item = char> {
        "".chars()
    }

    #[inline]
    fn chunks_in_byte_range(&self, range: ops::Range<usize>) -> impl Iterator<Item = &str> {
        iter::once(&self[range])
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
    fn get_line(&self, line_idx: usize) -> Option<Cow<'_, str>> {
        str_lines(self).nth(line_idx)
    }

    #[inline]
    fn get_char(&self, char_idx: usize) -> Option<char> {
        self.chars().nth(char_idx)
    }

    #[inline]
    fn line_to_char(&self, line_idx: usize) -> usize {
        str_lines(self).take(line_idx).map(|l| l.chars().count()).sum()
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
    fn char_to_line(&self, mut char_idx: usize) -> usize {
        // This should be a real assert, but it's expensive so we just return the last line
        // debug_assert!(char_idx < self.len_chars(), "char_idx out of bounds: {char_idx}");

        str_lines(self)
            .take_while(|l| {
                let n = l.chars().count();
                if n > char_idx {
                    return false;
                }
                char_idx -= n;
                true
            })
            .count()
    }

    #[inline]
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.chars().take(char_idx).map(|c| c.len_utf8()).sum()
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
