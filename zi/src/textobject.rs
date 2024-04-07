use std::{cmp, ops};

use crate::motion::Motion;
use crate::text::AnyText;

pub trait TextObject {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> ops::Range<usize>;
}

impl<M: Motion> TextObject for M {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, a: usize) -> ops::Range<usize> {
        let b = self.motion(text, a);

        match a.cmp(&b) {
            cmp::Ordering::Equal => a..a,
            cmp::Ordering::Less => a..b,
            cmp::Ordering::Greater => b..a,
        }
    }
}

pub struct Line;

impl TextObject for Line {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> ops::Range<usize> {
        let line_idx = text.byte_to_line(byte);
        let start = text.line_to_byte(line_idx);

        match text.try_line_to_byte(line_idx + 1) {
            Some(end) => start..end,
            // If the line is the last line, we want to include the previous newline
            None => start.saturating_sub(1)..text.len_bytes(),
        }
    }
}

#[cfg(test)]
mod tests;
