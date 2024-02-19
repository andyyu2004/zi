use zi::Editor;

#[test]
fn set_cursor() {
    let mut editor = Editor::new("foo\ntest\n");
    assert_eq!(editor.active_cursor(), (1, 0), "cursor should start at (1, 0)");
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.current_char(), 'f');

    editor.set_active_cursor((1, 1));
    assert_eq!(editor.active_cursor(), (1, 1));

    editor.set_active_cursor((1, 3));
    assert_eq!(editor.active_cursor(), (1, 2), "cursor should not move past end of line");

    editor.set_active_cursor((2, 2));
    assert_eq!(editor.active_cursor(), (2, 2));
    assert_eq!(editor.current_line(), "test\n");
    assert_eq!(editor.current_char(), 's');

    editor.set_active_cursor((2, 4));
    assert_eq!(editor.active_cursor(), (2, 3), "cursor should not move past end of line");

    editor.set_active_cursor((3, 0));
    assert_eq!(
        editor.active_cursor(),
        (3, 0),
        "this should be ok because the line exists (but is empty)"
    );

    editor.set_active_cursor((3, 1));
    assert_eq!(editor.active_cursor(), (3, 0));

    editor.set_active_cursor((4, 0));
    assert_eq!(editor.active_cursor(), (3, 0), "cursor should not move past end of buffer");
}
