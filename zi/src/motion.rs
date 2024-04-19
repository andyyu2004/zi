use std::{cmp, ops};

use crate::text::{AnyText, Text, TextSlice};
use crate::Point;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct NoMotion;

pub type MotionResult<T, E = NoMotion> = Result<T, E>;

pub trait Motion {
    /// Returns the new byte position after performing the motion starting at `byte`.
    /// Only `&self` is provided as the motion must not be stateful and should be able to be reused.
    /// The motion may choose to signal to the caller that no motion was possible by returning `Err(NoMotion)`.
    /// It's also valid to return the same byte position as the input.
    /// The caller may choose to handle them distinctly.
    fn motion(&self, text: &dyn AnyText, byte: usize) -> MotionResult<usize>;

    /// Returns the range of bytes that the motion would move over.
    /// This is allowed to have different behaviour than the default implemntation.
    // Perhaps the behaviour should be adjustable with a bunch of flags instead?
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> MotionResult<ops::Range<usize>> {
        let b = self.motion(text, byte)?;

        Ok(match byte.cmp(&b) {
            cmp::Ordering::Equal => byte..byte,
            cmp::Ordering::Less => byte..b,
            cmp::Ordering::Greater => b..byte,
        })
    }

    #[inline]
    fn point_motion(&self, text: &dyn AnyText, point: Point) -> MotionResult<Point> {
        let byte = self.motion(text, text.point_to_byte(point))?;
        Ok(text.byte_to_point(byte))
    }

    #[inline]
    fn repeated(self, n: usize) -> Repeated<Self>
    where
        Self: Sized,
    {
        Repeated { motion: self, n }
    }
}

impl<M: Motion> Motion for &M {
    #[inline]
    fn motion(&self, text: &dyn AnyText, byte: usize) -> MotionResult<usize> {
        (**self).motion(text, byte)
    }

    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> MotionResult<ops::Range<usize>> {
        (**self).byte_range(text, byte)
    }
}

pub struct Repeated<M> {
    motion: M,
    n: usize,
}

impl<M: Motion> Motion for Repeated<M> {
    fn motion(&self, text: &dyn AnyText, mut byte: usize) -> MotionResult<usize> {
        for _ in 0..self.n {
            byte = self.motion.motion(text, byte)?;
        }
        Ok(byte)
    }
}

fn copy<T: Copy>(&x: &T) -> T {
    x
}

trait CharExt {
    /// Returns true if the character is a word separator.
    #[allow(clippy::wrong_self_convention)]
    fn is_word_separator(self) -> bool;

    /// Returns true if the character is a token separator.
    #[allow(clippy::wrong_self_convention)]
    fn is_token_separator(self) -> bool;

    /// Returns true if the character is a word start.
    /// This includes non-alphanumeric characters and capital letters.
    #[allow(clippy::wrong_self_convention)]
    fn is_word_start(self) -> bool;
}

impl CharExt for char {
    #[inline]
    fn is_word_separator(self) -> bool {
        self.is_whitespace() || !self.is_alphanumeric()
    }

    #[inline]
    fn is_token_separator(self) -> bool {
        self.is_whitespace()
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
    fn motion(&self, text: &dyn AnyText, mut byte: usize) -> MotionResult<usize> {
        if byte == 0 {
            // To match `nvim` behaviour, when going back from the start of the buffer, we cancel the operation
            // https://github.com/neovim/neovim/blob/7fa24948a936a95519f0c8c496402488b6508c14/src/nvim/normal.c#L5874
            return Err(NoMotion);
        }

        let mut chars = text.byte_slice(..byte).chars().rev().peekable();

        let c = chars.peek().copied();
        let mut windows = chars.by_ref().map_windows::<_, _, 2>(copy).peekable();

        if windows.peek().is_none() {
            // If there is only one character left, then the windowed iterator is empty.
            // In this case, we just move back one character if possible.
            // Note that `c` must be saved before peeking the windows as that would consume it with
            // no way of getting it back.
            return Ok(byte - c.map_or(0, |c| c.len_utf8()));
        }

        while let Some([c, next]) = windows.next() {
            byte -= c.len_utf8();

            if matches!((c, next), ('\n', '\n')) {
                break;
            }

            // Stop if we're about to hit a separator or newline, or at a word start, unless We're currently on a separator.
            if ((self.is_sep)(next) || (self.is_start)(c, next)) && !(self.is_sep)(c) {
                break;
            }

            // last iteration of the loop, deal with the final character
            if windows.peek().is_none() {
                byte -= next.len_utf8();
            }
        }

        Ok(byte)
    }
}

pub struct PrevToken;

impl Motion for PrevToken {
    fn motion(&self, text: &dyn AnyText, byte: usize) -> MotionResult<usize> {
        Prev {
            is_sep: char::is_whitespace,
            is_start: |c, next| !c.is_whitespace() && next.is_whitespace(),
        }
        .motion(text, byte)
    }
}

pub struct NextWord;

impl NextWord {
    fn mv(&self, text: &dyn AnyText, mut byte: usize) -> (usize, bool) {
        let mut chars = text.byte_slice(byte..).chars();

        let Some(c) = chars.next() else { return (byte, false) };
        byte += c.len_utf8();

        if c == '\n' {
            // not even sure what the bool return is really meant to indicate anymore, but this needs to be
            // false to work :)
            return (byte, false);
        }

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
            if c == '\n' {
                return (byte, true);
            }
        }

        (byte, false)
    }
}

impl Motion for NextWord {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, start: usize) -> MotionResult<ops::Range<usize>> {
        let (end, just_crossed_newline) = self.mv(text, start);
        // Exclude the newline character if using as a range
        // e.g. dw does not delete the line break
        Ok(if just_crossed_newline { start..end.saturating_sub(1) } else { start..end })
    }

    #[inline]
    fn motion(&self, text: &dyn AnyText, byte: usize) -> MotionResult<usize> {
        Ok(self.mv(text, byte).0)
    }
}

pub struct PrevWord;

impl Motion for PrevWord {
    #[inline]
    fn motion(&self, text: &dyn AnyText, byte: usize) -> MotionResult<usize> {
        Prev { is_sep: char::is_word_separator, is_start: |c, _| c.is_word_start() }
            .motion(text, byte)
    }
}

/// Whitespace delimited word
pub struct NextToken;

impl NextToken {
    // from `:h w`
    // Another special case: When using the "w" motion in combination with an
    // operator and the last word moved over is at the end of a line, the end of
    // that word becomes the end of the operated text, not the first word in the
    // next line.
    fn mv(&self, text: &dyn AnyText, mut byte: usize, stop_before_newline: bool) -> usize {
        let chars = text.byte_slice(byte..).chars();

        let start_byte = byte;

        let mut found_sep = false;
        let mut prev_char = None;
        for c in chars {
            if found_sep && !c.is_token_separator() {
                break;
            }

            // empty lines are considered a word
            if prev_char == Some('\n') && c == '\n' {
                break;
            }

            if stop_before_newline && c == '\n' && byte != start_byte {
                break;
            }

            found_sep |= c.is_token_separator();
            byte += c.len_utf8();

            prev_char = Some(c);
        }

        assert!(byte > start_byte, "next_token motion should always move at least one byte");
        byte
    }
}

impl Motion for NextToken {
    #[inline]
    fn motion(&self, text: &dyn AnyText, byte: usize) -> MotionResult<usize> {
        Ok(self.mv(text, byte, false))
    }

    #[inline]
    fn byte_range(&self, text: &dyn AnyText, start: usize) -> MotionResult<ops::Range<usize>> {
        let end = self.mv(text, start, true);
        Ok(start..end)
    }
}

#[cfg(test)]
mod tests;
