//! Strongly prefer writing a test against nvim in `zi/tests/nvim/testdata`
//! This is useful for writing out cases with the debugger.

use super::*;

#[track_caller]
fn check(motion: &impl Motion, text: &str, byte: usize, expected: Result<usize, NoMotion>) {
    assert_eq!(motion.motion(&text, byte), expected);
}

#[test]
fn motion_prev_word() {
    let motion = PrevWord;
    check(&motion, "a", 0, Err(NoMotion));

    let text = "abc";
    for next_byte in [0, 0, 0] {
        check(&motion, text, 2, Ok(next_byte));
    }

    check(&motion, "a-A", 2, Ok(0));
}

#[test]
fn motion_next_word() {
    let motion = NextWord;
    check(&motion, "_a-", 0, Ok(1));
    check(&motion, "a-A", 0, Ok(2));
    check(&motion, "AAa", 0, Ok(1));
    check(&motion, "\n\n", 0, Ok(1));
    check(&motion, "\n\n", 1, Ok(2));

    check(&motion, "a\nb", 0, Ok(2));
    assert_eq!(motion.byte_range(&"a\nb", 0), Ok(0..1), "range should not include the newline");
}

#[test]
fn motion_next_token() {
    let motion = NextToken;
    check(&motion, " a", 0, Ok(1));
    check(&motion, "00", 0, Ok(2));
    check(&motion, "0", 0, Ok(1));
    check(&motion, "_a", 1, Ok(2));
    check(&motion, "\n\n", 0, Ok(1));
    check(&motion, "\n\n", 1, Ok(2));

    check(&motion, "a\nb", 0, Ok(2));
    assert_eq!(motion.byte_range(&"a\nb", 0), Ok(0..1), "range should not include the newline");

    check(&motion, "\na", 0, Ok(1));
    assert_eq!(motion.byte_range(&"\na", 0), Ok(0..1), "range should not become empty");
}

#[test]
fn motion_prev_token() {
    let motion = PrevToken;
    check(&motion, "01", 1, Ok(0));
    check(&motion, "-01", 2, Ok(0));
    check(&motion, "  A", 2, Ok(0));
    check(&motion, " Aa", 2, Ok(1));
}
