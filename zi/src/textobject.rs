use std::ops;

use crate::text::{AnyText, Text as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionKind {
    Linewise,
    Charwise,
}

pub trait TextObject {
    /// Returns the byte range of the text object that contains the given byte.
    /// To signal to the caller to cancel the operation, return `None`.
    /// It is also valid to return `Some(empty_range)` to proceed.
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>>;

    fn kind(&self) -> MotionKind;
}

impl<O: TextObject> TextObject for &O {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        (*self).byte_range(text, byte)
    }

    #[inline]
    fn kind(&self) -> MotionKind {
        (*self).kind()
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
    fn kind(&self) -> MotionKind {
        MotionKind::Linewise
    }

    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        let line_idx = text.byte_to_line(byte);
        let start = text.line_to_byte(line_idx);

        match self.0 {
            Inclusivity::Exclusive => {
                let len = text.get_line(line_idx).map_or(0, |line| line.len_bytes());
                Some(start..start + len)
            }
            Inclusivity::Inclusive => {
                match text.try_line_to_byte(line_idx + 1) {
                    Some(end) => Some(start..end),
                    // If the line is the last line, we want to include the previous newline
                    None => {
                        // We want to preserve the trailing newline if it exists
                        if text.chars().next_back() == Some('\n') {
                            Some(start.saturating_sub(1)..text.len_bytes() - 1)
                        } else {
                            Some(start.saturating_sub(1)..text.len_bytes())
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
