use super::*;

fn str_lines(s: &str) -> impl Iterator<Item = Cow<'_, str>> {
    // append an empty line if the string ends with a newline or is empty (to match ropey's behaviour)
    s.split_inclusive('\n').chain((s.is_empty() || s.ends_with('\n')).then_some("")).map(Into::into)
}

impl<'a> TextSlice<'a> for &'a str {
    type Slice = Self;

    fn to_cow(&self) -> Cow<'a, str> {
        Cow::Borrowed(*self)
    }

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self {
        <str as Text>::byte_slice(*self, byte_range)
    }

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice {
        let start = line_range.start_bound().map(|&l| self.line_to_byte(l));
        let end = line_range.end_bound().map(|&l| self.line_to_byte(l));
        <str as Text>::byte_slice(*self, (start, end))
    }

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> + 'a {
        str::chars(self)
    }

    fn lines(&self) -> impl Iterator<Item = Self> + 'a {
        str::lines(self)
    }

    fn get_line(&self, line_idx: usize) -> Option<Self> {
        str::lines(self).nth(line_idx)
    }

    fn chunks(&self) -> impl Iterator<Item = &'a str> + 'a {
        iter::once(*self)
    }
}

impl Text for str {
    type Slice<'a> = &'a str;

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        &self[(byte_range.start_bound().cloned(), byte_range.end_bound().cloned())]
    }

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        let start = line_range.start_bound().map(|&l| self.line_to_byte(l));
        let end = line_range.end_bound().map(|&l| self.line_to_byte(l));
        self.byte_slice((start, end))
    }

    #[inline]
    fn lines(&self) -> impl Iterator<Item = Self::Slice<'_>> {
        self.lines()
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        self.chars()
    }

    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.lines().nth(line_idx)
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
        self.lines().count()
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
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }
}
