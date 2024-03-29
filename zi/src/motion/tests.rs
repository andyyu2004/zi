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
}
