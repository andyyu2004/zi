use std::io::Write;

mod cursor;
mod insert;
mod picker;
mod scroll;
mod view;

/// Create a new editor with a buffer open with the given content.
pub fn new(content: &str) -> zi::Editor {
    new_with_size(content, zi::Size::new(80, 24))
}

pub fn new_with_size(content: &str, view_size: zi::Size) -> zi::Editor {
    let mut tmpfile = tempfile::NamedTempFile::new().expect("could not create tempfile");
    tmpfile.write_all(content.as_bytes()).expect("could not write to tempfile");
    let (mut editor, ..) = zi::Editor::new(zi::Size::new(
        view_size.width,
        view_size.height + zi::Editor::BOTTOM_BAR_HEIGHT,
    ));
    let buf = editor.open_active(tmpfile.path()).expect("failed to open buffer");
    assert_eq!(editor.active_buffer().id(), buf);
    // not asserting equality as we may have added a trailing newline to the end of the file
    assert!(editor.active_buffer().text().to_string().starts_with(content));
    editor
}
