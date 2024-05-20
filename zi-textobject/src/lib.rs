#![feature(iter_map_windows)]

pub mod motion;
use std::ops;

use zi_core::PointOrByte;
use zi_text::{AnyText, Text as _, TextSlice};

pub use self::motion::Motion;

/// Charwise textobjects affect a [start, end) byte-range where `start` is inclusive and `end` is exclusive.
/// Linewise ranges will NOT be expanded to include the full start and end lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjectKind {
    Linewise,
    Charwise,
}

impl TextObjectKind {
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
        const EXCLUSIVE = 1 << 0;
        const UPDATE_TARGET_COLUMN = 1 << 1;
    }
}

pub trait TextObject {
    /// Returns the byte range of the text object that contains the given byte.
    /// To signal to the caller to cancel the operation, return `None`.
    /// It is also valid to return `Some(empty_range)` to proceed.
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>>;

    fn default_kind(&self) -> TextObjectKind;

    #[inline]
    fn flags(&self) -> TextObjectFlags {
        TextObjectFlags::empty()
    }
}

impl TextObject for &dyn Motion {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        (**self).byte_range(text, byte)
    }

    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        (**self).default_kind()
    }

    #[inline]
    fn flags(&self) -> TextObjectFlags {
        (**self).flags()
    }
}

impl<O: TextObject> TextObject for &O {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        (*self).byte_range(text, byte)
    }

    #[inline]
    fn default_kind(&self) -> TextObjectKind {
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
pub struct Line {
    inclusivity: Inclusivity,
}

impl Line {
    pub fn new(inclusivity: Inclusivity) -> Self {
        Self { inclusivity }
    }

    pub fn inclusive() -> Self {
        Self::new(Inclusivity::Inclusive)
    }

    pub fn exclusive() -> Self {
        Self::new(Inclusivity::Exclusive)
    }
}

impl TextObject for Line {
    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Linewise
    }

    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        let line = text.byte_to_line(byte);
        Some(line_range_to_byte_range(text, line..=line, self.inclusivity))
    }
}

pub struct Repeat<M> {
    pub(crate) inner: M,
    pub(crate) n: usize,
}

impl<M: TextObject> TextObject for Repeat<M> {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        let _ = (text, byte);
        todo!();
    }

    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        self.inner.default_kind()
    }

    #[inline]
    fn flags(&self) -> TextObjectFlags {
        self.inner.flags()
    }
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

pub struct PrevToken;

impl PrevToken {
    fn imp() -> Prev {
        Prev {
            is_sep: char::is_whitespace,
            is_start: |c, next| !c.is_whitespace() && next.is_whitespace(),
        }
    }
}

impl TextObject for PrevToken {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        Self::imp().byte_range(text, byte)
    }

    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }

    #[inline]
    fn flags(&self) -> TextObjectFlags {
        TextObjectFlags::EXCLUSIVE | TextObjectFlags::UPDATE_TARGET_COLUMN
    }
}

pub struct NextWord;

impl NextWord {
    pub(crate) fn mv(&self, text: &dyn AnyText, p: impl Into<PointOrByte>) -> (usize, bool) {
        let mut byte = text.point_or_byte_to_byte(p.into());
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

impl TextObject for NextWord {
    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }

    #[inline]
    fn byte_range(&self, text: &dyn AnyText, start: usize) -> Option<ops::Range<usize>> {
        let (end, just_crossed_newline) = self.mv(text, start);
        // Exclude the newline character if using as a range
        // e.g. dw does not delete the line break
        Some(if just_crossed_newline { start..end.saturating_sub(1) } else { start..end })
    }

    #[inline]
    fn flags(&self) -> TextObjectFlags {
        TextObjectFlags::EXCLUSIVE
    }
}

pub struct PrevWord;

impl PrevWord {
    fn imp() -> Prev {
        Prev { is_sep: char::is_word_separator, is_start: |c, _| c.is_word_start() }
    }
}

impl TextObject for PrevWord {
    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }

    #[inline]
    fn byte_range(&self, text: &dyn AnyText, start: usize) -> Option<ops::Range<usize>> {
        Self::imp().byte_range(text, start)
    }

    #[inline]
    fn flags(&self) -> TextObjectFlags {
        TextObjectFlags::EXCLUSIVE | TextObjectFlags::UPDATE_TARGET_COLUMN
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
    pub(crate) fn mv(
        &self,
        text: &dyn AnyText,
        p: impl Into<PointOrByte>,
        stop_before_newline: bool,
    ) -> usize {
        let mut byte = text.point_or_byte_to_byte(p.into());
        let mut chars = text.byte_slice(byte..).chars().peekable();

        match chars.peek() {
            Some('\n') if stop_before_newline => {
                assert_eq!(chars.next(), Some('\n'));
                // if the cursor is at the end of a line, the next word is the first word of the next line.
                // However, we don't want to chop the trailing newline if there is one.
                return byte + chars.peek().is_some() as usize;
            }
            _ => (),
        }

        let start_byte = byte;

        let mut found_sep = false;
        let mut prev_char = None;
        while let Some(c) = chars.next() {
            if found_sep && !c.is_token_separator() {
                break;
            }

            // empty lines are considered a word
            if prev_char == Some('\n') && c == '\n' {
                break;
            }

            if stop_before_newline && c == '\n' && (byte > start_byte || chars.peek().is_none()) {
                break;
            }

            found_sep |= c.is_token_separator();
            byte += c.len_utf8();

            prev_char = Some(c);
        }

        byte
    }
}

impl TextObject for NextToken {
    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }

    #[inline]
    fn byte_range(&self, text: &dyn AnyText, start: usize) -> Option<ops::Range<usize>> {
        Some(start..self.mv(text, start, true))
    }

    #[inline]
    fn flags(&self) -> TextObjectFlags {
        TextObjectFlags::EXCLUSIVE
    }
}

struct Prev {
    is_sep: fn(char) -> bool,
    is_start: fn(char, char) -> bool,
}

impl TextObject for Prev {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        if byte == 0 {
            // To match `nvim` behaviour, when going back from the start of the buffer, we cancel the operation
            // https://github.com/neovim/neovim/blob/7fa24948a936a95519f0c8c496402488b6508c14/src/nvim/normal.c#L5874
            return None;
        }

        Some(self.motion(text, byte.into()).try_into_byte().unwrap()..byte)
    }

    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }
}

pub struct NextChar;

impl TextObject for NextChar {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        Some(byte..self.motion(text, byte.into()).try_into_byte().unwrap())
    }

    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }
}

pub struct PrevChar;

impl TextObject for PrevChar {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        Some(self.motion(text, byte.into()).try_into_byte().unwrap()..byte)
    }

    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }
}

pub struct NextLine;

impl TextObject for NextLine {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        let line_idx = text.byte_to_line(byte);
        if line_idx + 1 == text.len_lines() {
            return None;
        }
        Some(line_range_to_byte_range(text, line_idx..=line_idx + 1, Inclusivity::Inclusive))
    }

    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Linewise
    }
}

pub struct PrevLine;

impl TextObject for PrevLine {
    #[inline]
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        let line_idx = text.byte_to_line(byte);
        Some(line_range_to_byte_range(
            text,
            line_idx.checked_sub(1)?..=line_idx,
            Inclusivity::Inclusive,
        ))
    }

    #[inline]
    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Linewise
    }
}

fn line_range_to_byte_range(
    text: &dyn AnyText,
    line_range: ops::RangeInclusive<usize>,
    inclusivity: Inclusivity,
) -> ops::Range<usize> {
    let start_line = *line_range.start();
    let end_line = *line_range.end();
    let start = text.line_to_byte(start_line);
    match inclusivity {
        Inclusivity::Exclusive => {
            let (skip, len) = text.line(end_line).map_or((0, 0), |line| {
                // We want to exclude the leading whitespace when in exclusive mode
                let skip = line
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .map(|c| c.len_utf8())
                    .sum::<usize>();
                (skip, line.len_bytes())
            });
            start + skip..start + len
        }
        Inclusivity::Inclusive => {
            match text.try_line_to_byte(end_line + 1) {
                Some(end) => start..end,
                // If the line is the last line, we want to include the trailing newline
                None => start..text.len_bytes(),
            }
        }
    }
}
