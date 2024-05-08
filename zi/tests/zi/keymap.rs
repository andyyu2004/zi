use zi::input::KeySequence;

use crate::api::new;

#[test]
fn composite_escape() {
    #[track_caller]
    fn check(seq: &str, expectation: &str) {
        let mut editor = new("");
        let seq = seq.parse::<KeySequence>().unwrap();
        for key in seq {
            editor.handle_input(key);
        }

        assert_eq!(editor.cursor_line(), expectation);
    }

    check("ifd", "");
    check("iffk", "ffk");
    check("iffd", "f");
    check("ifffx", "fffx");
}
