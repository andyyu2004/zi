use super::*;

#[derive(Debug, Clone)]
pub struct Until(pub char);

impl TextObject for Until {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        let mut end = byte;
        let chars = text.byte_slice(byte..).chars();

        for c in chars {
            if c == self.0 {
                return Some(byte..end);
            }

            end += c.len_utf8();
        }

        None
    }

    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }
}
