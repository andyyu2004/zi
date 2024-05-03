//! Prefer writing a test against nvim in `zi/tests/nvim/testdata`
//! This is useful for writing out cases with the debugger.
use std::ops;

use zi_text::Rope;
use zi_textobject::*;

#[track_caller]
fn check_range(
    obj: &impl TextObject,
    text: &str,
    byte: usize,
    expected: Option<ops::Range<usize>>,
) {
    assert_eq!(obj.byte_range(&text, byte), expected);
    assert_eq!(obj.byte_range(&Rope::from(text), byte), expected);
}

#[test]
fn line_object() {
    let inc = Line::inclusive();
    let exl = Line::exclusive();

    check_range(&inc, "\n", 0, Some(0..1));
    check_range(&inc, "\n", 1, Some(1..1));

    check_range(&inc, "\n\n", 0, Some(0..1));
    check_range(&inc, "\n\n", 1, Some(1..2));

    check_range(&exl, "\n", 0, Some(0..0));
    check_range(&exl, "\n", 1, Some(1..1));

    check_range(&inc, "abc\nd", 0, Some(0..4));
    check_range(&exl, "abc\nd", 0, Some(0..3));

    check_range(&exl, " ab", 1, Some(1..3));
    check_range(&exl, "  ab", 1, Some(2..4));
    check_range(&exl, "  ab ", 1, Some(2..5));
    check_range(&exl, "  ab ", 0, Some(2..5));

    check_range(&inc, "\n\n\n", 2, Some(2..3));
}

#[track_caller]
fn check(motion: &impl Motion, text: &str, byte: usize, expected: usize) {
    assert_eq!(motion.motion(&text, byte), expected);
    assert_eq!(motion.motion(&Rope::from(text), byte), expected);
}

fn prev_token_and_word(motion: &impl Motion) {
    check(&motion, "ab\nc", 3, 0);
    check(&motion, "a\n\nc", 3, 2);
    check(&motion, "a\nb", 2, 0);
    // skips the whitespace until after the first newline
    check(&motion, "a\n\n c", 4, 2);

    check(&motion, "a\n \nb", 4, 2);

    check_range(&motion, "a b", 2, Some(0..2));
}

#[test]
fn motion_prev_word() {
    let motion = PrevWord;

    prev_token_and_word(&motion);

    check(&motion, "a", 0, 0);

    let text = "abc";
    for next_byte in [0, 0, 0] {
        check(&motion, text, 2, next_byte);
    }

    check(&motion, "a-A", 2, 0);
    check(&motion, ":aA", 2, 1);
    check(&motion, "$aA", 2, 1);
}

#[test]
fn motion_prev_token() {
    let motion = PrevToken;
    prev_token_and_word(&motion);
    check(&motion, "01", 1, 0);
    check(&motion, "-01", 2, 0);
    check(&motion, "  A", 2, 0);
    check(&motion, " Aa", 2, 1);
}

fn next_token_and_word(motion: &impl Motion) {
    check(&motion, "a\n c", 0, 3);
    check(&motion, "a\nc", 0, 2);
    check(&motion, "a\nb", 0, 2);
    check(&motion, "a\nb", 2, 3);

    // should not delete the trailing newline
    check_range(&motion, "\n", 0, Some(0..0));
    check_range(&motion, "\n\n", 1, Some(1..1));
    check_range(&motion, "\n\n\n", 2, Some(2..2));

    // should stop at the newline
    check_range(&motion, "\n a", 0, Some(0..1));

    // but should delete non-trailing newlines
    check_range(&motion, "\n\n", 0, Some(0..1));

    // range should exclude the newline
    check_range(&motion, "a\nb", 0, Some(0..1));
    // range should not become empty
    check_range(&motion, "\na", 0, Some(0..1));
}

#[test]
fn motion_next_word() {
    let motion = NextWord;
    // next_token_and_word(&motion); FIXME TODO TODO HELP!
    check(&motion, "_a-", 0, 1);
    check(&motion, "a-A", 0, 2);
    check(&motion, "AAa", 0, 1);
    check(&motion, "\n\n", 0, 1);
    check(&motion, "\n\n", 1, 2);

    check(&motion, "a\nb", 0, 2);
}

#[test]
fn motion_next_token() {
    let motion = NextToken;
    next_token_and_word(&motion);
    check(&motion, " a", 0, 1);
    check(&motion, "00", 0, 2);
    check(&motion, "0", 0, 1);
    check(&motion, "_a", 1, 2);
    check(&motion, "\n\n", 0, 1);
    check(&motion, "\n\n", 1, 2);

    check(&motion, "\n a", 0, 2);

    check(&motion, "a\nb", 0, 2);

    check(&motion, "\na", 0, 1);
}

#[test]
fn motion_next_char() {
    let motion = NextChar;
    check(&motion, "a", 0, 1);
    check(&motion, "ab", 0, 1);
    check(&motion, "a", 1, 1);
    check(&motion, "a\nb", 0, 1);
    check(&motion, "\n\n\n", 1, 1);
    check(&motion, "\n\n\n", 0, 0);

    check_range(&motion, "a", 0, Some(0..1));
}

#[test]
fn motion_prev_char() {
    let motion = PrevChar;

    check(&motion, "a", 1, 0);
    check(&motion, "---------\u{a002d}-\u{fe2c}\0---\u{a05cc}\n", 25, 21)
}
