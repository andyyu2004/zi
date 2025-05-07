use std::ops::Range;

use zi_text::AnyText;

use crate::delimiter::Delimiter;
use crate::{TextObject, TextObjectKind, Within};

pub struct Around<D>(pub D);

impl<D: Delimiter> TextObject for Around<D> {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<Range<usize>> {
        let mut range = Within(&self.0).byte_range(text, byte)?;
        range.start -= D::OPEN.len_utf8();
        range.end += D::CLOSE.len_utf8();
        Some(range)
    }

    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }
}
