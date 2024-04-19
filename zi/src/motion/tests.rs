//! Strongly prefer writing a test against nvim in `zi/tests/nvim/testdata`
//! This is useful for writing out cases with the debugger.

use super::*;

#[track_caller]
fn check(motion: &impl Motion, text: &str, byte: usize, expected: Result<usize, NoMotion>) {
    assert_eq!(motion.motion(&text, byte), expected);
}

#[track_caller]
fn check_range(
    motion: &impl Motion,
    text: &str,
    byte: usize,
    expected: Result<ops::Range<usize>, NoMotion>,
) {
    assert_eq!(motion.byte_range(&text, byte), expected);
}

fn prev_token_and_word(motion: &impl Motion) {
    check(&motion, "a\n\nc", 3, Ok(2));
    check(&motion, "a\nb", 2, Ok(0));
    // skips the whitespace to after the first newline
    check(&motion, "a\n\n c", 4, Ok(2));
}

#[test]
fn motion_prev_word() {
    let motion = PrevWord;

    prev_token_and_word(&motion);

    check(&motion, "a", 0, Err(NoMotion));

    let text = "abc";
    for next_byte in [0, 0, 0] {
        check(&motion, text, 2, Ok(next_byte));
    }

    check(&motion, "a-A", 2, Ok(0));
    check(&motion, ":aA", 2, Ok(1));
    check(&motion, "$aA", 2, Ok(1));
}

#[test]
fn motion_prev_token() {
    let motion = PrevToken;
    prev_token_and_word(&motion);
    check(&motion, "01", 1, Ok(0));
    check(&motion, "-01", 2, Ok(0));
    check(&motion, "  A", 2, Ok(0));
    check(&motion, " Aa", 2, Ok(1));
}

fn next_token_and_word(motion: &impl Motion) {
    check(&motion, "a\n c", 0, Ok(3));
    check(&motion, "a\nc", 0, Ok(2));
    check(&motion, "a\nb", 0, Ok(2));
    // range should exclude the newline
    check_range(&motion, "a\nb", 0, Ok(0..1));
    // range should not become empty
    check_range(&motion, "\na", 0, Ok(0..1));
}

#[test]
fn motion_next_word() {
    let motion = NextWord;
    // next_token_and_word(&motion); FIXME
    check(&motion, "_a-", 0, Ok(1));
    check(&motion, "a-A", 0, Ok(2));
    check(&motion, "AAa", 0, Ok(1));
    check(&motion, "\n\n", 0, Ok(1));
    check(&motion, "\n\n", 1, Ok(2));

    check(&motion, "a\nb", 0, Ok(2));
}

#[test]
fn motion_next_token() {
    let motion = NextToken;
    next_token_and_word(&motion);
    check(&motion, " a", 0, Ok(1));
    check(&motion, "00", 0, Ok(2));
    check(&motion, "0", 0, Ok(1));
    check(&motion, "_a", 1, Ok(2));
    check(&motion, "\n\n", 0, Ok(1));
    check(&motion, "\n\n", 1, Ok(2));

    check(&motion, "a\nb", 0, Ok(2));

    check(&motion, "\na", 0, Ok(1));
}
