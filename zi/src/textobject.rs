use std::ops;

use crate::motion::{Motion, MotionResult};
use crate::text::{AnyText, Text as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjectKind {
    Linewise,
    Charwise,
}

pub trait TextObject {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> MotionResult<ops::Range<usize>>;

    #[inline]
    fn kind(&self) -> TextObjectKind {
        // TODO may not always be true
        TextObjectKind::Charwise
    }
}

impl<M: Motion> TextObject for M {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, a: usize) -> MotionResult<ops::Range<usize>> {
        self.byte_range(text, a)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inclusivity {
    Exclusive,
    Inclusive,
}

/// A text object that represents a line.
/// The line can be include or exclude the newline character.
pub struct Line(pub Inclusivity);

impl TextObject for Line {
    #[inline]
    fn kind(&self) -> TextObjectKind {
        TextObjectKind::Linewise
    }

    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> MotionResult<ops::Range<usize>> {
        let line_idx = text.byte_to_line(byte);
        let start = text.line_to_byte(line_idx);

        match self.0 {
            Inclusivity::Exclusive => {
                let len = text.get_line(line_idx).map_or(0, |line| line.len_bytes());
                Ok(start..start + len)
            }
            Inclusivity::Inclusive => {
                match text.try_line_to_byte(line_idx + 1) {
                    Some(end) => Ok(start..end),
                    // If the line is the last line, we want to include the previous newline
                    None => Ok(start.saturating_sub(1)..text.len_bytes()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
