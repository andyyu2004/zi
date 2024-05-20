use super::new;

#[test]
fn picker() {
    let mut editor = new("");

    assert_eq!(editor.views().count(), 1);
    let current_buf = editor.buffer(zi::Active).id();

    editor.open_file_picker(".");
    assert_ne!(
        editor.buffer(zi::Active).id(),
        current_buf,
        "view should be focused on new buffer of picker"
    );

    editor.close_view(zi::Active);
    assert_eq!(
        editor.buffer(zi::Active).id(),
        current_buf,
        "view should be focused on original buffer after closing picker"
    );

    assert_eq!(editor.views().count(), 1, "the picker views should be gone");

    editor.open_file_picker(".");
    assert_eq!(
        editor.views().count(),
        4,
        "the file picker consists of 3 views (picker, results, and preview)"
    );

    editor.open_file_picker(".");
    assert_eq!(
        editor.views().count(),
        4,
        "opening another picker should not create new set of views"
    );
}

#[test]
fn picker_input() {
    let mut editor = new("");
    editor.open_jump_list(zi::Active);
    editor.input("abc").unwrap();
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "abc");
    assert_eq!(editor.cursor(zi::Active), (0, 3));

    editor.input("<BS>").unwrap();
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "ab");
    assert_eq!(editor.cursor(zi::Active), (0, 2));
}
