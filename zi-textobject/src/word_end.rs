//! `e`, `ge`, `E` and `gE`.
//!
//! These follow [vim-wordmotion](https://github.com/chaoren/vim-wordmotion), which the nvim tests
//! load, with its default settings except that the uppercase motions are mapped, with WORDs
//! separated by whitespace alone.
//!
//! Both work the way vim's `search()` does with the `e` flag and `c` in `cpoptions`: split each
//! line into non-overlapping matches from its start, then move to the end of the next (or
//! previous) match.
use std::borrow::Cow;
use std::ops;

use zi_core::PointOrByte;
use zi_text::{AnyText, Text as _, TextSlice as _};

use super::{Motion, TextObject, TextObjectFlags, TextObjectKind};

/// A match in a line, as a byte range relative to the start of the line. It may be empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    start: usize,
    end: usize,
}

impl Span {
    /// The column vim compares with the cursor to decide whether the match is before or after it:
    /// the start of its last character, or the column before an empty match.
    fn compare_col(self, line: &str) -> isize {
        if self.start == self.end {
            self.start.cast_signed() - 1
        } else {
            last_char_start(line, self.end).cast_signed()
        }
    }

    /// The column the cursor lands on.
    fn landing_col(self, line: &str) -> usize {
        if self.start == self.end {
            // vim puts the cursor at the start of an empty match. Normal mode can't put it past
            // the end of the line.
            if self.start < line.len() { self.start } else { last_char_start(line, line.len()) }
        } else {
            last_char_start(line, self.end)
        }
    }
}

/// The start of the last character before `end`.
fn last_char_start(line: &str, end: usize) -> usize {
    line[..end].char_indices().next_back().map_or(0, |(i, _)| i)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Word,
    Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Forward,
    Backward,
}

struct LineInfo<'a> {
    text: Cow<'a, str>,
    is_first: bool,
    is_last: bool,
}

fn line_info(text: &dyn AnyText, line_idx: usize) -> Option<LineInfo<'_>> {
    let line = text.line(line_idx)?;
    let mut s = line.to_cow();
    if s.ends_with('\n') {
        match &mut s {
            Cow::Borrowed(b) => *b = &b[..b.len() - 1],
            Cow::Owned(o) => {
                o.pop();
            }
        }
    }
    Some(LineInfo { text: s, is_first: line_idx == 0, is_last: line_idx + 1 >= text.len_lines() })
}

fn spans(kind: Kind, dir: Direction, line: &LineInfo<'_>) -> Vec<Span> {
    match kind {
        Kind::Word => word_spans(dir, line),
        Kind::Token => token_spans(dir, line),
    }
}

/// WORDs: runs of non-blank characters. Going backwards, an empty line counts as a WORD too.
fn token_spans(dir: Direction, line: &LineInfo<'_>) -> Vec<Span> {
    let text = &*line.text;
    let mut spans = vec![];
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (start, c.is_whitespace()) {
            (None, false) => start = Some(i),
            (Some(s), true) => {
                spans.push(Span { start: s, end: i });
                start = None;
            }
            (None, true) | (Some(_), false) => {}
        }
    }
    if let Some(s) = start {
        spans.push(Span { start: s, end: text.len() });
    }

    match dir {
        Direction::Forward => {}
        Direction::Backward => {
            if text.is_empty() {
                spans.push(Span { start: 0, end: 0 });
            }
        }
    }

    spans
}

fn is_keyword(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || (!c.is_ascii() && c.is_alphabetic())
}

/// Whether the character at `i` is a space according to wordmotion: whitespace, a run of `-`
/// between letters, or a run of `_` between letters or digits.
fn is_space(chars: &[char], i: usize) -> bool {
    let c = chars[i];
    if c.is_whitespace() {
        return true;
    }

    let between = |sep: char, word: fn(&char) -> bool| {
        if c != sep {
            return false;
        }
        let before = chars[..i].iter().rev().find(|&&c| c != sep);
        let after = chars[i + 1..].iter().find(|&&c| c != sep);
        before.is_some_and(word) && after.is_some_and(word)
    };

    between('-', char::is_ascii_alphabetic) || between('_', char::is_ascii_alphanumeric)
}

/// Length in chars of the match of wordmotion's word pattern at `i`, trying its alternatives in
/// order as vim's regex engine does.
fn word_match_at(chars: &[char], i: usize, space: &[bool]) -> Option<usize> {
    let at = |j: usize| chars.get(j).copied();
    let count =
        |from: usize, f: fn(&char) -> bool| chars[from..].iter().take_while(|c| f(c)).count();
    let is_kw_at = |j: usize| at(j).is_some_and(is_keyword);
    let word_end_at = |j: usize| is_kw_at(j.wrapping_sub(1)) && !is_kw_at(j);
    let word_start_at = |j: usize| !is_kw_at(j.wrapping_sub(1)) && is_kw_at(j);

    let c = at(i)?;

    // CamelCase
    if c.is_ascii_uppercase() {
        let lower = count(i + 1, char::is_ascii_lowercase);
        if lower > 0 {
            return Some(1 + lower);
        }
    }

    // UPPERCASE, not followed by a lowercase letter (backtracking if it is)
    let upper = count(i, char::is_ascii_uppercase);
    if upper > 0 {
        let next_is_lower = at(i + upper).is_some_and(|c| c.is_ascii_lowercase());
        let len = if next_is_lower { upper - 1 } else { upper };
        if len > 0 {
            return Some(len);
        }
    }

    // lowercase
    let lower = count(i, char::is_ascii_lowercase);
    if lower > 0 {
        return Some(lower);
    }

    // #0F0F0F
    let hex_colour_at = |j: usize| {
        at(j) == Some('#') && {
            let n = count(j + 1, char::is_ascii_hexdigit);
            n > 0 && word_end_at(j + 1 + n)
        }
    };
    if hex_colour_at(i) {
        return Some(1 + count(i + 1, char::is_ascii_hexdigit));
    }

    // 0x00, 0o00, 0b00
    if c == '0' && word_start_at(i) {
        let digits: Option<fn(&char) -> bool> = match at(i + 1) {
            Some('x' | 'X') => Some(char::is_ascii_hexdigit),
            Some('o' | 'O') => Some(|c| matches!(c, '0'..='7')),
            Some('b' | 'B') => Some(|c| matches!(c, '0' | '1')),
            _ => None,
        };
        if let Some(digits) = digits {
            let n = count(i + 2, digits);
            if n > 0 && word_end_at(i + 2 + n) {
                return Some(2 + n);
            }
        }
    }

    // digits
    let digits = count(i, char::is_ascii_digit);
    if digits > 0 {
        return Some(digits);
    }

    // other printable characters
    let other = (i..chars.len())
        .take_while(|&j| {
            let c = chars[j];
            let hash_before_hex = c == '#' && at(j + 1).is_some_and(|c| c.is_ascii_hexdigit());
            !c.is_control() && !c.is_ascii_alphanumeric() && !space[j] && !hash_before_hex
        })
        .count();
    if other > 0 {
        return Some(other);
    }

    None
}

fn word_spans(dir: Direction, line: &LineInfo<'_>) -> Vec<Span> {
    let text = &*line.text;
    let chars = text.chars().collect::<Vec<_>>();
    let space = (0..chars.len()).map(|i| is_space(&chars, i)).collect::<Vec<_>>();
    let byte_offsets =
        text.char_indices().map(|(i, _)| i).chain(std::iter::once(text.len())).collect::<Vec<_>>();

    let mut spans = vec![];
    let mut i = 0;
    while i <= chars.len() {
        if let Some(len) = word_match_at(&chars, i, &space) {
            spans.push(Span { start: byte_offsets[i], end: byte_offsets[i + len] });
            i += len;
            continue;
        }

        // The empty alternatives: start of file, end of file, and an empty line (not for `e`).
        let empty_line = text.is_empty() && matches!(dir, Direction::Backward);
        if (i == 0 && line.is_first) || (i == chars.len() && line.is_last) || empty_line {
            spans.push(Span { start: byte_offsets[i], end: byte_offsets[i] });
        }
        i += 1;
    }

    spans
}

/// Returns the byte the motion moves to, or `None` if there is no match.
fn search(text: &dyn AnyText, byte: usize, kind: Kind, dir: Direction) -> Option<usize> {
    if text.len_lines() == 0 {
        return None;
    }

    let byte = byte.min(text.len_bytes());
    let line_idx = text.byte_to_line(byte).min(text.len_lines() - 1);
    let line_start = text.line_to_byte(line_idx);
    let col = (byte - line_start).cast_signed();

    let land = |line_idx: usize, line: &LineInfo<'_>, span: Span| {
        text.line_to_byte(line_idx) + span.landing_col(&line.text)
    };

    match dir {
        Direction::Forward => {
            let line = line_info(text, line_idx)?;
            if let Some(&span) =
                spans(kind, dir, &line).iter().find(|s| s.compare_col(&line.text) > col)
            {
                return Some(land(line_idx, &line, span));
            }

            (line_idx + 1..text.len_lines()).find_map(|idx| {
                let line = line_info(text, idx)?;
                let &span = spans(kind, dir, &line).first()?;
                Some(land(idx, &line, span))
            })
        }
        Direction::Backward => {
            // A backward search from the first column starts on the line above.
            if col > 0 {
                let line = line_info(text, line_idx)?;
                if let Some(&span) =
                    spans(kind, dir, &line).iter().rev().find(|s| s.compare_col(&line.text) < col)
                {
                    return Some(land(line_idx, &line, span));
                }
            }

            (0..line_idx).rev().find_map(|idx| {
                let line = line_info(text, idx)?;
                let &span = spans(kind, dir, &line).last()?;
                Some(land(idx, &line, span))
            })
        }
    }
}

macro_rules! word_end_motion {
    ($(#[$attr:meta])* $name:ident, $kind:expr, $dir:expr) => {
        $(#[$attr])*
        pub struct $name;

        impl Motion for $name {
            fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
                self.motion_n(text, p, 1)
            }

            fn motion_n(&self, text: &dyn AnyText, p: PointOrByte, n: usize) -> PointOrByte {
                target(text, text.point_or_byte_to_byte(p), $kind, $dir, n).into()
            }
        }

        impl TextObject for $name {
            fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
                self.byte_range_n(text, byte, 1)
            }

            fn byte_range_n(
                &self,
                text: &dyn AnyText,
                byte: usize,
                n: usize,
            ) -> Option<ops::Range<usize>> {
                if text.is_empty() {
                    return None;
                }

                let target = target(text, byte, $kind, $dir, n);
                let (start, end) = match $dir {
                    Direction::Forward => (byte, target),
                    Direction::Backward => (target, byte),
                };
                // The motions are inclusive and forced charwise: the range covers the character
                // under `end`, including the line break of an empty line, but not the one ending
                // the last line.
                let end = match text.char_at_byte(end) {
                    None => end,
                    Some('\n') if end + 1 == text.len_bytes() => end,
                    Some(c) => end + c.len_utf8(),
                };
                Some(start..end.min(text.len_bytes()))
            }

            fn default_kind(&self) -> TextObjectKind {
                TextObjectKind::Charwise
            }

            fn flags(&self) -> TextObjectFlags {
                TextObjectFlags::UPDATE_TARGET_COLUMN | TextObjectFlags::INCLUSIVE
            }
        }
    };
}

word_end_motion!(
    /// `e`: the end of the next word.
    NextWordEnd,
    Kind::Word,
    Direction::Forward
);

word_end_motion!(
    /// `ge`: the end of the previous word.
    PrevWordEnd,
    Kind::Word,
    Direction::Backward
);

word_end_motion!(
    /// `E`: the end of the next whitespace delimited word.
    NextTokenEnd,
    Kind::Token,
    Direction::Forward
);

word_end_motion!(
    /// `gE`: the end of the previous whitespace delimited word.
    PrevTokenEnd,
    Kind::Token,
    Direction::Backward
);

/// Searches `n` times, stopping at the first search that finds nothing. A failed search leaves
/// the cursor where it is, but an operator still covers the character under it.
fn target(text: &dyn AnyText, mut byte: usize, kind: Kind, dir: Direction, n: usize) -> usize {
    let start = byte;
    for _ in 0..n {
        match search(text, byte, kind, dir) {
            Some(next) => byte = next,
            None => break,
        }
    }
    // wordmotion moves `ge` and `gE` to the start of the first line if the cursor didn't move.
    match dir {
        Direction::Backward if byte == start && text.byte_to_line(byte) == 0 => 0,
        Direction::Forward | Direction::Backward => byte,
    }
}
