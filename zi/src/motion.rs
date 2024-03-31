use std::ops;

use crate::text::{AnyText, Text, TextSlice};
use crate::Point;

pub trait Motion {
    /// Returns the new byte position after performing the motion.
    /// Only `&self` is provided as the motion must not be stateful and should be able to be reused.
    fn motion(&self, text: &dyn AnyText, byte: usize) -> usize;

    #[inline]
    fn point_motion(&self, text: &dyn AnyText, point: Point) -> Point {
        let byte = self.motion(text, text.point_to_byte(point));
        text.byte_to_point(byte)
    }

    #[inline]
    fn byte_range(&self, text: &dyn AnyText, start: usize) -> ops::Range<usize> {
        let end = self.motion(text, start);
        tracing::error!(x = ?(start..end));
        start..end
    }

    fn repeated(self, n: usize) -> Repeated<Self>
    where
        Self: Sized,
    {
        Repeated { motion: self, n }
    }
}

impl<M: Motion> Motion for &M {
    fn motion(&self, text: &dyn AnyText, byte: usize) -> usize {
        (**self).motion(text, byte)
    }
}

pub struct Repeated<M> {
    motion: M,
    n: usize,
}

impl<M: Motion> Motion for Repeated<M> {
    fn motion(&self, text: &dyn AnyText, mut byte: usize) -> usize {
        for _ in 0..self.n {
            byte = self.motion.motion(text, byte);
        }
        byte
    }
}

fn copy<T: Copy>(&x: &T) -> T {
    x
}

trait CharExt {
    /// Returns true if the character is a word separator. This is whitespace, `-`, and `_`.
    #[allow(clippy::wrong_self_convention)]
    fn is_word_separator(self) -> bool;

    /// Returns true if the character is a word start.
    /// This includes non-alphanumeric characters and capital letters.
    #[allow(clippy::wrong_self_convention)]
    fn is_word_start(self) -> bool;
}

impl CharExt for char {
    #[inline]
    fn is_word_separator(self) -> bool {
        self.is_whitespace() || matches!(self, '-' | '_')
    }

    #[inline]
    fn is_word_start(self) -> bool {
        (self.is_uppercase() || !self.is_alphanumeric()) && !self.is_word_separator()
    }
}

struct Prev {
    is_sep: fn(char) -> bool,
    is_start: fn(char, char) -> bool,
}

impl Motion for Prev {
    fn motion(&self, text: &dyn AnyText, mut byte: usize) -> usize {
        let mut chars = text.byte_slice(..byte).chars().rev().peekable();

        let c = chars.peek().copied();
        let mut windows = chars.by_ref().map_windows::<_, _, 2>(copy).peekable();

        if windows.peek().is_none() {
            // If there is only one character left, then the windowed iterator is empty.
            // In this case, we just move back one character if possible.
            // Note that `c` must be saved before peeking the windows as that would consume it with
            // no way of getting it back.
            return byte - c.map_or(0, |c| c.len_utf8());
        }

        while let Some([c, next]) = windows.next() {
            byte -= c.len_utf8();

            if ((self.is_sep)(next) || (self.is_start)(c, next))
                && (!(self.is_sep)(c) || !(self.is_sep)(next))
            {
                break;
            }

            // last iteration of the loop, deal with the final character
            if windows.peek().is_none() {
                byte -= next.len_utf8();
            }
        }

        byte
    }
}

pub struct PrevToken;

impl Motion for PrevToken {
    fn motion(&self, text: &dyn AnyText, byte: usize) -> usize {
        Prev {
            is_sep: char::is_whitespace,
            is_start: |c, next| !c.is_whitespace() && next.is_whitespace(),
        }
        .motion(text, byte)
    }
}

pub struct NextWord;

impl Motion for NextWord {
    fn motion(&self, text: &dyn AnyText, mut byte: usize) -> usize {
        let mut chars = text.byte_slice(byte..).chars();

        let Some(c) = chars.next() else { return byte };
        byte += c.len_utf8();

        let mut found_sep = c.is_word_separator();
        for c in chars {
            let is_sep = c.is_word_separator();
            if found_sep && !is_sep || c.is_word_start() {
                break;
            }

            if c.is_word_separator() {
                found_sep = true;
            }

            byte += c.len_utf8();
        }

        byte
    }
}

pub struct PrevWord;

impl Motion for PrevWord {
    fn motion(&self, text: &dyn AnyText, byte: usize) -> usize {
        Prev { is_sep: char::is_word_separator, is_start: |c, _| c.is_word_start() }
            .motion(text, byte)
    }
}

/// Whitespace delimited word
pub struct NextToken;

impl Motion for NextToken {
    fn motion(&self, text: &dyn AnyText, mut byte: usize) -> usize {
        let chars = text.byte_slice(byte..).chars();

        let mut found_whitespace = false;
        for c in chars {
            if found_whitespace && !c.is_whitespace() {
                break;
            }

            found_whitespace |= c.is_whitespace();
            byte += c.len_utf8();
        }

        byte
    }
}

#[cfg(test)]
mod tests;
