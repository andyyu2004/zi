use zi::input::KeyEvent;

use crate::api::new;

fn k(s: &str) -> KeyEvent {
    s.parse().unwrap()
}

#[test]
fn test_composite_escape() {
    let mut editor = new("");
    editor.handle_input(k("i"));
    for i in 0..5 {
        editor.handle_input(k("f"));
        assert_eq!(editor.current_line(), format!("{}", "f".repeat(i + 1)));
    }
    editor.handle_input(k("d"));
    assert_eq!(editor.current_line(), "fffff");
}
