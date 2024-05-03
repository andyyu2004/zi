use super::*;

impl TextBase for Cow<'_, str> {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }

    #[inline]
    fn len_lines(&self) -> usize {
        self.as_ref().len_lines()
    }

    #[inline]
    fn len_bytes(&self) -> usize {
        self.as_ref().len_bytes()
    }

    #[inline]
    fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.as_ref().byte_to_line(byte_idx)
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.as_ref().line_to_byte(line_idx)
    }

    #[inline]
    fn get_char(&self, byte_idx: usize) -> Option<char> {
        self.as_ref().get_char(byte_idx)
    }
}
