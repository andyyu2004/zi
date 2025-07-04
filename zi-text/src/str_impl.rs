use std::io;

use super::*;

fn str_lines_inclusive(s: &str) -> impl Iterator<Item = &str> {
    // TODO CRLF?
    s.split_inclusive('\n')
}

impl<'a> TextSlice<'a> for &'a str {
    type Slice = Self;
    type Chunks = std::iter::Once<Self>;

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

    fn lines(&self) -> impl DoubleEndedIterator<Item = Self> + 'a {
        Text::lines(*self)
    }

    fn line(&self, line_idx: usize) -> Option<Self> {
        Text::line(*self, line_idx)
    }

    fn chunks(&self) -> Self::Chunks {
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
    fn line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        Text::lines(self).nth(line_idx)
    }

    #[inline]
    fn reader(&self) -> impl Read + '_ {
        io::Cursor::new(self)
    }
}

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
    fn len_utf16_cu(&self) -> usize {
        self.chars().map(char::len_utf16).sum()
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

    #[inline]
    fn byte_to_utf16_cu(&self, byte_idx: usize) -> usize {
        self[..byte_idx].chars().map(char::len_utf16).sum()
    }

    #[inline]
    fn utf16_cu_to_byte(&self, mut cu_idx: usize) -> usize {
        let mut chars = self.chars();
        let mut byte_idx = 0;
        while cu_idx > 0 {
            let cu = chars.next().expect("cu_idx out of bounds");
            byte_idx += cu.len_utf8();
            cu_idx = cu_idx.checked_sub(cu.len_utf16()).expect("cu_idx was not on a char boundary");
        }
        byte_idx
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
    fn len_utf16_cu(&self) -> usize {
        self.as_str().len_utf16_cu()
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

    #[inline]
    fn byte_to_utf16_cu(&self, byte_idx: usize) -> usize {
        self.as_str().byte_to_utf16_cu(byte_idx)
    }

    #[inline]
    fn utf16_cu_to_byte(&self, cu_idx: usize) -> usize {
        self.as_str().utf16_cu_to_byte(cu_idx)
    }
}

impl Text for String {
    type Slice<'a>
        = &'a str
    where
        Self: 'a;

    #[inline]
    fn byte_slice(&self, byte_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        self.as_str().byte_slice(byte_range)
    }

    #[inline]
    fn line_slice(&self, line_range: impl RangeBounds<usize>) -> Self::Slice<'_> {
        self.as_str().line_slice(line_range)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        self.as_str().chars()
    }

    #[inline]
    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice<'_>> {
        self.as_str().lines()
    }

    #[inline]
    fn line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.as_str().line(line_idx)
    }

    #[inline]
    fn reader(&self) -> impl Read + '_ {
        self.as_str().reader()
    }
}

impl TextMut for String {
    fn edit(&mut self, deltas: &Deltas<'_>) -> Deltas<'static> {
        deltas.apply(self)
    }
}
