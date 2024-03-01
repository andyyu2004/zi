use zi::input::KeyEvent;

use crate::api::new;

fn k(s: &str) -> KeyEvent {
    s.parse().unwrap()
}

#[test]
fn test_composite_escape() {
    #[track_caller]
    fn check(seq: &str, expectation: &str) {
        let mut editor = new("");
        for c in seq.chars() {
            let ev = k(&c.to_string());
            editor.handle_input(ev);
        }

        assert_eq!(editor.current_line(), expectation);
    }

    check("ifd", "");
    check("iffk", "ffk");
    check("iffd", "f");
    check("ifffx", "fffx");
}
