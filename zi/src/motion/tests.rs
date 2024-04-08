//! Strongly prefer writing a test against nvim in `zi/tests/nvim/testdata`
//! This is useful for writing out cases with the debugger.

use super::*;

#[test]
fn motion_prev_word() {
    let motion = PrevWord;
    assert_eq!(motion.motion(&"a", 0), 0);

    let text = "abc";
    for next_byte in [0, 0, 0] {
        assert_eq!(motion.motion(&text, 2), next_byte);
    }

    assert_eq!(motion.motion(&"a-A", 2), 0);
}

#[test]
fn motion_next_word() {
    let motion = NextWord;
    assert_eq!(motion.motion(&"_a-", 0), 1);
    assert_eq!(motion.motion(&"a-A", 0), 2);
    assert_eq!(motion.motion(&"AAa", 0), 1);
    assert_eq!(motion.motion(&"\n\n", 0), 1, "newline should be a word separator");
    assert_eq!(motion.motion(&"\n\n", 1), 2, "newline should be a word separator");

    assert_eq!(motion.motion(&"a\nb", 0), 2);
    assert_eq!(motion.byte_range(&"a\nb", 0), 0..1, "range should not include the newline");
}

#[test]
fn motion_next_token() {
    let motion = NextToken;
    assert_eq!(motion.motion(&" a", 0), 1);
    assert_eq!(motion.motion(&"00", 0), 2);
    assert_eq!(motion.motion(&"0", 0), 1);
    assert_eq!(motion.motion(&"_a", 1), 2);
    assert_eq!(motion.motion(&"\n\n", 0), 1, "newline should be a word separator");
    assert_eq!(motion.motion(&"\n\n", 1), 2, "newline should be a word separator");

    assert_eq!(motion.motion(&"a\nb", 0), 2);
    assert_eq!(motion.byte_range(&"a\nb", 0), 0..1, "range should not include the newline");

    assert_eq!(motion.motion(&"\na", 0), 1);
    assert_eq!(motion.byte_range(&"\na", 0), 0..1, "range should not become empty");
}

#[test]
fn motion_prev_token() {
    let motion = PrevToken;
    assert_eq!(motion.motion(&"01", 1), 0);
    assert_eq!(motion.motion(&"-01", 2), 0);
    assert_eq!(motion.motion(&"  A", 2), 0);
    assert_eq!(motion.motion(&" Aa", 2), 1);
}
