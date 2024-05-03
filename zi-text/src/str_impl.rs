use super::*;

fn str_lines_inclusive(s: &str) -> impl Iterator<Item = &str> {
    // TODO CRLF?
    s.split_inclusive('\n')
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
        Text::lines(*self)
    }

    fn get_line(&self, line_idx: usize) -> Option<Self> {
        Text::get_line(*self, line_idx)
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
    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice<'_>> {
        self.lines()
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        self.chars()
    }

    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        Text::lines(self).nth(line_idx)
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
        str_lines_inclusive(self).take(line_idx).map(|l| l.len()).sum()
    }

    fn byte_to_line(&self, mut byte_idx: usize) -> usize {
        assert!(byte_idx <= self.len(), "byte_idx out of bounds: {byte_idx}");
        // some special cases to match `crop::Rope`
        if byte_idx == self.len() {
            return if self.ends_with('\n') {
                self.lines().count()
            } else {
                self.lines().count().saturating_sub(1)
            };
        }

        str_lines_inclusive(self)
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

    #[inline]
    fn get_char(&self, byte_idx: usize) -> Option<char> {
        if byte_idx >= self.len() {
            return None;
        }

        self[byte_idx..].chars().next()
    }
}

impl TextBase for String {
    #[inline]
    fn len_bytes(&self) -> usize {
        self.as_str().len()
    }

    #[inline]
    fn len_lines(&self) -> usize {
        self.as_str().len_lines()
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.as_str().line_to_byte(line_idx)
    }

    fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.as_str().byte_to_line(byte_idx)
    }

    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        Some(self)
    }

    #[inline]
    fn get_char(&self, byte_idx: usize) -> Option<char> {
        self.as_str().get_char(byte_idx)
    }
}

impl Text for String {
    type Slice<'a> = &'a str
    where
        Self: 'a;

    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        self.as_str().byte_slice(byte_range)
    }

    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        self.as_str().line_slice(line_range)
    }

    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        self.as_str().chars()
    }

    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice<'_>> {
        self.as_str().lines()
    }

    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.as_str().get_line(line_idx)
    }
}

impl TextMut for String {
    fn edit(&mut self, delta: &Delta<'_>) -> Delta<'static> {
        delta.apply(self)
    }
}
