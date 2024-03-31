use std::{cmp, ops};

use crate::motion::Motion;
use crate::text::{AnyText, Text};

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

pub struct CurrentLine;

impl TextObject for CurrentLine {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> ops::Range<usize> {
        let line_idx = text.byte_to_line(byte);
        let start = text.line_to_byte(line_idx);

        let end = if text.len_lines() <= 1 {
            let line = text.get_line(line_idx).unwrap_or_else(|| Box::new(""));
            start + line.len_bytes()
        } else {
            text.line_to_byte(line_idx + 1)
        };

        start..end
    }
}
