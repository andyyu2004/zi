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

    /// The motion repeated `n` times.
    fn motion_n(&self, text: &dyn AnyText, mut p: PointOrByte, n: usize) -> PointOrByte {
        for _ in 0..n {
            p = self.motion(text, p);
        }
        p
    }

    #[inline]
    fn motion_flags(&self) -> MotionFlags {
        MotionFlags::empty()
    }
}

impl Motion for &dyn Motion {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        (**self).motion(text, p)
    }

    #[inline]
    fn motion_n(&self, text: &dyn AnyText, p: PointOrByte, n: usize) -> PointOrByte {
        (**self).motion_n(text, p, n)
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

    #[inline]
    fn motion_n(&self, text: &dyn AnyText, p: PointOrByte, n: usize) -> PointOrByte {
        (**self).motion_n(text, p, n)
    }
}

impl<M: Motion> Motion for Repeat<M> {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        self.inner.motion_n(text, p, self.n)
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
        Self::mv(text, p).0.into()
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
        Self::mv(text, p, false).into()
    }
}

impl Motion for Prev {
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        let mut byte = text.point_or_byte_to_byte(p);
        let mut graphemes = graphemes_before(text, byte);
        let Some(Grapheme { first: mut c, mut len }) = graphemes.next() else {
            return byte.into();
        };

        for Grapheme { first: next, len: next_len } in graphemes {
            byte -= len;

            if matches!((c, next), ('\n', '\n')) {
                return byte.into();
            }

            // Stop if we're about to hit a separator or newline, or at a word start, unless we're currently on a separator.
            if ((self.is_sep)(next) || (self.is_start)(c, next)) && (!(self.is_sep)(c)) {
                return byte.into();
            }

            c = next;
            len = next_len;
        }

        (byte - len).into()
    }
}

impl Motion for NextChar {
    #[inline]
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        let byte = text.point_or_byte_to_byte(p);
        match graphemes_from(text, byte).next() {
            Some(Grapheme { first, len }) if first != '\n' => byte + len,
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
        match graphemes_before(text, byte).next() {
            Some(Grapheme { first, len }) if first != '\n' => byte - len,
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
        if text.line(point.line() + 1).is_none() {
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

pub struct StartOfLine;

impl Motion for StartOfLine {
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        let line_idx = text.byte_to_line(text.point_or_byte_to_byte(p));
        let start_byte = text.line_to_byte(line_idx);
        let line = text.line(line_idx).unwrap();
        (start_byte
            + line.chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum::<usize>())
        .into()
    }
}

impl TextObject for StartOfLine {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        let start = text.point_or_byte_to_byte(self.motion(text, byte.into()));
        assert!(start <= byte, "start of line should be before or at the current byte");
        Some(start..byte)
    }

    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Linewise
    }
}
