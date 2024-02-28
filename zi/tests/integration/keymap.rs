use zi::input::KeyEvent;

use crate::api::new;

fn k(s: &str) -> KeyEvent {
    s.parse().unwrap()
}

#[test]
fn test_composite_escape() {
    let mut editor = new("");
    editor.handle_key_event(k("i"));
    for i in 0..5 {
        editor.handle_key_event(k("f"));
        assert_eq!(editor.current_line(), format!("{}", "f".repeat(i + 1)));
    }
    editor.handle_key_event(k("d"));
    assert_eq!(editor.current_line(), "fffff");
}
