use zi_core::PointOrByte;
use zi_text::{AnyText, Text, TextSlice};

pub use super::*;

bitflags::bitflags! {
    pub struct MotionFlags: u8 {
        const NO_FORCE_UPDATE_TARGET = 1 << 0;
        const USE_TARGET_COLUMN = 1 << 1 | Self::NO_FORCE_UPDATE_TARGET.bits();
    }
}

/// Motions are a subset of textobjects that move the cursor around.
// TODO could probably write this in a more combinator-like style
pub trait Motion: TextObject {
    /// Returns the new byte position after performing the motion starting at `byte`.
    /// Only `&self` is provided as the motion must not be stateful and should be able to be reused.
    /// The motion may choose to signal to the caller that no motion was possible by returning `Err(NoMotion)`.
    /// It's also valid to return the same byte position as the input.
    /// The caller may choose to handle them distinctly.
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte;

    #[inline]
    fn motion_flags(&self) -> MotionFlags {
        MotionFlags::empty()
    }

    #[inline]
    fn repeat(self, n: usize) -> Repeat<Self>
    where
        Self: Sized,
    {
        Repeat { inner: self, n }
    }
}

impl Motion for &dyn Motion {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        (**self).motion(text, p)
    }

    #[inline]

    fn motion_flags(&self) -> MotionFlags {
        (**self).motion_flags()
    }
}

impl<M: Motion> Motion for &M {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        (**self).motion(text, p)
    }
}

impl<M: Motion> Motion for Repeat<M> {
    #[inline]
    fn motion(&self, text: &dyn AnyText, mut p: PointOrByte) -> PointOrByte {
        for _ in 0..self.n {
            p = self.inner.motion(text, p);
        }

        p
    }

    #[inline]
    fn motion_flags(&self) -> MotionFlags {
        self.inner.motion_flags()
    }
}

impl Motion for PrevToken {
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        Self::imp().motion(text, p)
    }
}

impl Motion for NextWord {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        self.mv(text, p).0.into()
    }
}

impl Motion for PrevWord {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        Self::imp().motion(text, p)
    }
}

impl Motion for NextToken {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        self.mv(text, p, false).into()
    }
}

impl Motion for Prev {
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        let mut byte = text.point_or_byte_to_byte(p);
        let mut chars = text.byte_slice(..byte).chars().rev().peekable();

        let c = chars.peek().copied();
        let mut windows = chars.by_ref().map_windows::<_, _, 2>(|c| *c).peekable();

        if windows.peek().is_none() {
            // If there is only one character left, then the windowed iterator is empty.
            // In this case, we just move back one character if possible.
            // Note that `c` must be saved before peeking the windows as that would consume it with
            // no way of getting it back.
            return (byte - c.map_or(0, |c| c.len_utf8())).into();
        }

        while let Some([c, next]) = windows.next() {
            byte -= c.len_utf8();

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

        byte.into()
    }
}

impl Motion for NextChar {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        let byte = text.point_or_byte_to_byte(p);
        match text.char_at_byte(byte) {
            Some(c) if c != '\n' => byte + c.len_utf8(),
            _ => byte,
        }
        .into()
    }

    #[inline]
    fn motion_flags(&self) -> MotionFlags {
        MotionFlags::NO_FORCE_UPDATE_TARGET
    }
}

impl Motion for PrevChar {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        let byte = text.point_or_byte_to_byte(p);
        match text.char_before_byte(byte) {
            Some(c) if c != '\n' => byte - c.len_utf8(),
            _ => byte,
        }
        .into()
    }

    #[inline]
    fn motion_flags(&self) -> MotionFlags {
        MotionFlags::NO_FORCE_UPDATE_TARGET
    }
}

impl Motion for NextLine {
    #[inline]
    fn motion(&self, text: &dyn AnyText, point: PointOrByte) -> PointOrByte {
        let point = text.point_or_byte_to_point(point);
        if point.line() == text.len_lines() {
            return point.into();
        }

        point.down(1).into()
    }

    #[inline]
    fn motion_flags(&self) -> MotionFlags {
        MotionFlags::USE_TARGET_COLUMN
    }
}

impl Motion for PrevLine {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        let point = text.point_or_byte_to_point(p);
        if point.line() == 0 {
            return point.into();
        }

        point.up(1).into()
    }

    #[inline]
    fn motion_flags(&self) -> MotionFlags {
        MotionFlags::USE_TARGET_COLUMN
    }
}
