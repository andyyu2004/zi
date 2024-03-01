use super::new;

#[test]
fn test_picker() {
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
}
