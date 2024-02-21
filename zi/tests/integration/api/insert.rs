#[test]
fn test_insert() {
    let mut editor = zi::Editor::new("");
    assert_eq!(editor.active_cursor(), (0, 0));
    editor.set_mode(zi::Mode::Insert);
    assert_eq!(editor.active_cursor(), (0, 0));

    assert_eq!(editor.active_cursor(), (0, 0));
    editor.insert_char('a');
    assert_eq!(editor.active_cursor(), (0, 1));
    editor.insert_char('b');
    assert_eq!(editor.active_cursor(), (0, 2));
    assert_eq!(editor.current_line(), "ab");

    editor.set_mode(zi::Mode::Normal);
    assert_eq!(editor.active_cursor(), (0, 1), "insert mode should move cursor left on exit");

    editor.set_active_cursor((1, 0));
    editor.set_mode(zi::Mode::Insert);
    editor.set_mode(zi::Mode::Normal);
    assert_eq!(editor.active_cursor(), (0, 0), "nowhere left to move");
}
