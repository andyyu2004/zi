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

        assert_eq!(editor.current_line(), expectation);
    }

    check("ifd", "\n");
    check("iffk", "ffk\n");
    check("iffd", "f\n");
    check("ifffx", "fffx\n");
}
