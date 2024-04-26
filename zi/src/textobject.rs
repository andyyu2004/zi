use std::ops;

use crate::text::{AnyText, Text as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionKind {
    Linewise,
    Charwise,
}

impl MotionKind {
    #[must_use]
    pub fn is_linewise(&self) -> bool {
        matches!(self, Self::Linewise)
    }

    #[must_use]
    pub fn is_charwise(&self) -> bool {
        matches!(self, Self::Charwise)
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TextObjectFlags: u8 {
        /// This is used to help match neovim behaviour. There are certain special cases
        /// that only apply to exclusive text objects. This does not affect what `byte_range`
        /// should return.
        const EXCLUSIVE = 0b0001;
    }
}

pub trait TextObject {
    /// Returns the byte range of the text object that contains the given byte.
    /// To signal to the caller to cancel the operation, return `None`.
    /// It is also valid to return `Some(empty_range)` to proceed.
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>>;

    fn default_kind(&self) -> MotionKind;

    #[inline]
    fn flags(&self) -> TextObjectFlags {
        TextObjectFlags::empty()
    }
}

impl<O: TextObject> TextObject for &O {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        (*self).byte_range(text, byte)
    }

    #[inline]
    fn default_kind(&self) -> MotionKind {
        (*self).default_kind()
    }

    #[inline]
    fn flags(&self) -> TextObjectFlags {
        (*self).flags()
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
    fn default_kind(&self) -> MotionKind {
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
