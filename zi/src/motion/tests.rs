//! Strongly prefer writing a test against nvim in `zi/tests/nvim/testdata`
//! This is useful for writing out difficult cases with the debugger.

use super::*;

#[test]
fn motion_prev_word() {
    assert_eq!(PrevWord.motion(&"a", 0), 0);

    let text = "@bc d@f gh@ !";
    let mut motion = PrevWord;

    let mut byte = text.len() - 1;
    for next_byte in [10, 8, 6, 5, 4, 1, 0, 0, 0] {
        assert_eq!(motion.motion(&text, byte), next_byte);
        byte = next_byte;
    }

    let text = "abc";
    for next_byte in [0, 0, 0] {
        assert_eq!(motion.motion(&text, 2), next_byte);
    }
}

#[test]
fn motion_next_word() {
    let text = "AୟX";
    assert_eq!(NextWord.motion(&text, 0), 1);
    assert_eq!(NextWord.motion(&text, 1), 4);
}
