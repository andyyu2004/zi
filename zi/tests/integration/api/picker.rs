use super::new;

#[test]
fn picker() {
    let mut editor = new("");
    let current_buf = editor.active_buffer().id();
    editor.open_file_picker(".");
    assert_ne!(
        editor.active_buffer().id(),
        current_buf,
        "view should be focused on new buffer of picker"
    );

    editor.close_active_view();
    assert_eq!(
        editor.active_buffer().id(),
        current_buf,
        "view should be focused on original buffer after closing picker"
    );

    assert_eq!(editor.views().len(), 1, "the picker view should be gone");

    editor.open_file_picker(".");
    assert_eq!(editor.views().len(), 2);

    editor.open_file_picker(".");
    assert_eq!(editor.views().len(), 2, "opening another picker should not create a new view");
}
