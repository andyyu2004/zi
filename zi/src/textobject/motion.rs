use zi_text::{AnyText, Text, TextSlice};

pub use super::*;
use crate::Point;

/// Motions are a subset of textobjects that move the cursor around.
// TODO could probably write this in a more combinator-like style
pub trait Motion: TextObject {
    /// Returns the new byte position after performing the motion starting at `byte`.
    /// Only `&self` is provided as the motion must not be stateful and should be able to be reused.
    /// The motion may choose to signal to the caller that no motion was possible by returning `Err(NoMotion)`.
    /// It's also valid to return the same byte position as the input.
    /// The caller may choose to handle them distinctly.
    fn motion(&self, text: &dyn AnyText, byte: usize) -> usize;

    #[inline]
    fn point_motion(&self, text: &dyn AnyText, point: Point) -> Point {
        let byte = self.motion(text, text.point_to_byte(point));
        text.byte_to_point(byte)
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
    fn motion(&self, text: &dyn AnyText, byte: usize) -> usize {
        (**self).motion(text, byte)
    }
}

impl<M: Motion> Motion for Repeated<M> {
    fn motion(&self, text: &dyn AnyText, mut byte: usize) -> usize {
        for _ in 0..self.n {
            byte = self.motion.motion(text, byte);
        }

        byte
    }
}

impl Motion for PrevToken {
    fn motion(&self, text: &dyn AnyText, byte: usize) -> usize {
        Self::imp().motion(text, byte)
    }
}

impl Motion for NextWord {
    #[inline]
    fn motion(&self, text: &dyn AnyText, byte: usize) -> usize {
        self.mv(text, byte).0
    }
}

impl Motion for PrevWord {
    #[inline]
    fn motion(&self, text: &dyn AnyText, byte: usize) -> usize {
        Self::imp().motion(text, byte)
    }
}

impl Motion for NextToken {
    #[inline]
    fn motion(&self, text: &dyn AnyText, byte: usize) -> usize {
        self.mv(text, byte, false)
    }
}

impl Motion for Prev {
    fn motion(&self, text: &dyn AnyText, mut byte: usize) -> usize {
        let mut chars = text.byte_slice(..byte).chars().rev().peekable();

        let c = chars.peek().copied();
        let mut windows = chars.by_ref().map_windows::<_, _, 2>(|c| *c).peekable();

        if windows.peek().is_none() {
            // If there is only one character left, then the windowed iterator is empty.
            // In this case, we just move back one character if possible.
            // Note that `c` must be saved before peeking the windows as that would consume it with
            // no way of getting it back.
            return byte - c.map_or(0, |c| c.len_utf8());
        }

        let mut crossed_newline = false;

        while let Some([c, next]) = windows.next() {
            byte -= c.len_utf8();

            if crossed_newline && next == '\n' {
                // should never cross two newlines
                break;
            }

            crossed_newline |= c == '\n';

            if matches!((c, next), ('\n', '\n')) {
                break;
            }

            // Stop if we're about to hit a separator or newline, or at a word start, unless we're currently on a separator.
            if ((self.is_sep)(next) || (self.is_start)(c, next)) && (!(self.is_sep)(c)) {
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
