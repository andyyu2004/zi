//! Strongly prefer writing a test against nvim in `zi/tests/nvim/testdata`
//! This is useful for writing out cases with the debugger.

use super::*;

#[track_caller]
fn check(motion: &impl Motion, text: &str, byte: usize, expected: usize) {
    assert_eq!(motion.motion(&text, byte), expected);
}

// These are textobject tests now, should probably move them
#[track_caller]
fn check_range(motion: &impl Motion, text: &str, byte: usize, expected: Option<ops::Range<usize>>) {
    assert_eq!(motion.byte_range(&text, byte), expected);
}

fn prev_token_and_word(motion: &impl Motion) {
    check(&motion, "a\n\nc", 3, 2);
    check(&motion, "a\nb", 2, 0);
    // skips the whitespace to after the first newline
    check(&motion, "a\n\n c", 4, 2);
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

    // but should delete non-trailing newlines
    check_range(&motion, "\n\n", 0, Some(0..1));

    // range should exclude the newline
    check_range(&motion, "a\nb", 0, Some(0..1));
    // range should not become empty
    // check_range(&motion, "\na", 0, Some(0..1));
}

#[test]
fn motion_next_word() {
    let motion = NextWord;
    // next_token_and_word(&motion); FIXME
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

    check(&motion, "a\nb", 0, 2);

    check(&motion, "\na", 0, 1);
}
