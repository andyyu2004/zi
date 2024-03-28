use crate::text::{AnyText, Text, TextSlice};
use crate::Point;

pub trait Motion {
    fn motion(&mut self, text: &dyn AnyText, byte: usize) -> usize;

    #[inline]
    fn point_motion(&mut self, text: &dyn AnyText, point: Point) -> Point {
        let byte = self.motion(text, text.point_to_byte(point));
        text.byte_to_point(byte)
    }

    fn repeated(self, n: usize) -> Repeated<Self>
    where
        Self: Sized,
    {
        Repeated { motion: self, n }
    }
}

pub struct Repeated<M> {
    motion: M,
    n: usize,
}

impl<M: Motion> Motion for Repeated<M> {
    fn motion(&mut self, text: &dyn AnyText, mut byte: usize) -> usize {
        for _ in 0..self.n {
            byte = self.motion.motion(text, byte);
        }
        byte
    }
}

pub struct PrevToken;

impl Motion for PrevToken {
    fn motion(&mut self, text: &dyn AnyText, mut byte: usize) -> usize {
        let mut chars = text.byte_slice(..byte).chars();

        let prev = chars.next_back().unwrap_or('x');
        for c in chars.rev() {
            if c.is_whitespace() && !prev.is_whitespace() {
                break;
            }
            byte -= c.len_utf8();
        }

        byte
    }
}

pub struct NextWord;

impl Motion for NextWord {
    fn motion(&mut self, text: &dyn AnyText, mut byte: usize) -> usize {
        let mut chars = text.byte_slice(byte..).chars();

        let Some(c) = chars.next() else { return byte };
        byte += c.len_utf8();

        let is_special = |c: char| !c.is_alphanumeric();

        if is_special(c) {
            // If we were on a separator, then we just move a character.
            return byte;
        }

        let mut found_whitespace = false;
        for c in chars {
            if found_whitespace && !c.is_whitespace() {
                break;
            }

            if c.is_whitespace() {
                found_whitespace = true;
            } else if is_special(c) {
                break;
            }

            byte += c.len_utf8();
        }

        byte
    }
}

fn copy<T: Copy>(&x: &T) -> T {
    x
}

trait CharExt {
    fn is_word_boundary(&self) -> bool;
}

impl CharExt for char {
    fn is_word_boundary(&self) -> bool {
        self.is_whitespace() || !self.is_alphanumeric()
    }
}

pub struct PrevWord;

impl Motion for PrevWord {
    fn motion(&mut self, text: &dyn AnyText, mut byte: usize) -> usize {
        let mut chars = text.byte_slice(..byte).chars().rev().peekable();

        let c = chars.peek().copied();

        let mut windows = chars.by_ref().map_windows(copy).peekable();
        if windows.peek().is_none() {
            // If there is only one character left, then the windowed iterator is empty.
            // In this case, we just move back one character if possible.
            // Note that `c` must be saved before peeking the windows as that would consume it with
            // no way of getting it back.
            return byte - c.map_or(0, |c| c.len_utf8());
        }

        while let Some([c, next]) = windows.next() {
            byte -= c.len_utf8();

            if (next.is_word_boundary() || !c.is_alphanumeric()) && !c.is_whitespace() {
                break;
            }

            // last iteration of the loop, deal with the final character
            if windows.peek().is_none() && !next.is_word_boundary() {
                byte -= next.len_utf8();
            }
        }

        byte
    }
}

/// Whitespace delimited word
pub struct NextToken;

impl Motion for NextToken {
    fn motion(&mut self, text: &dyn AnyText, mut byte: usize) -> usize {
        let chars = text.byte_slice(byte..).chars();

        let mut found_whitespace = false;
        for c in chars {
            if found_whitespace && !c.is_whitespace() {
                break;
            }

            if c.is_whitespace() {
                found_whitespace = true;
            }

            byte += c.len_utf8();
        }

        byte
    }
}

#[cfg(test)]
mod tests;
