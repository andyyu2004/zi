use std::ops;

use crate::motion::{Motion, MotionResult};
use crate::text::AnyText;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjectKind {
    Linewise,
    Charwise,
}

pub trait TextObject {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> MotionResult<ops::Range<usize>>;

    #[inline]
    fn kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }
}

impl<M: Motion> TextObject for M {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, a: usize) -> MotionResult<ops::Range<usize>> {
        self.byte_range(text, a)
    }
}

pub struct Line;

impl TextObject for Line {
    #[inline]
    fn kind(&self) -> TextObjectKind {
        TextObjectKind::Linewise
    }

    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> MotionResult<ops::Range<usize>> {
        let line_idx = text.byte_to_line(byte);
        let start = text.line_to_byte(line_idx);

        match text.try_line_to_byte(line_idx + 1) {
            Some(end) => Ok(start..end),
            // If the line is the last line, we want to include the previous newline
            None => Ok(start.saturating_sub(1)..text.len_bytes()),
        }
    }
}

#[cfg(test)]
mod tests;
