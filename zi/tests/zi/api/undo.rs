use super::new;

#[test]
fn undo_insertion() {
    let mut editor = new("x");
    editor.set_mode(zi::Mode::Insert);
    for c in ['a', 'b', 'c'] {
        editor.insert_char_at_cursor(c);
    }

    editor.set_mode(zi::Mode::Normal);
    assert_eq!(editor.current_line(), "abcx");

    editor.undo(zi::Active);
    assert_eq!(editor.current_line(), "x");

    editor.redo(zi::Active);
    assert_eq!(editor.current_line(), "abcx");
}

#[test]
fn undo_textobject_deletion() {
    let mut editor = new("abc");
    editor.input("dd").unwrap();
    assert_eq!(editor.current_line(), "");
    editor.undo(zi::Active);
    assert_eq!(editor.current_line(), "abc");
}

#[test]
fn undo_textobject_deletion_tmp() {
    let mut editor = new("A");

    editor.set_cursor(zi::Active, (0, 0));
    editor.set_mode(zi::Mode::Normal);
    editor.clear_undo();

    editor.handle_input('d');
    editor.handle_input('W');
    assert_eq!(editor.current_line(), "");
    editor.handle_input('u');
    assert_eq!(editor.current_line(), "A");
}
