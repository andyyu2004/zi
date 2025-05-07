use std::ops::Range;

use zi_text::{AnyText, Text as _, TextSlice as _};

use crate::delimiter::Delimiter;
use crate::{TextObject, TextObjectKind};

pub struct Within<D>(pub D);

impl<D: Delimiter> TextObject for Within<D> {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<Range<usize>> {
        let mut start = byte;
        let mut chars = text.byte_slice(..byte).chars().rev();
        loop {
            let c = chars.next()?;
            if c == D::OPEN {
                break;
            }

            start -= c.len_utf8();
        }

        let mut end = byte;
        let mut chars = text.byte_slice(byte..).chars();
        loop {
            let c = chars.next()?;
            if c == D::CLOSE {
                break;
            }

            end += c.len_utf8();
        }

        Some(start..end)
    }

    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }
}
