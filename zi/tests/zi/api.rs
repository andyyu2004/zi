use std::io::Write;

use expect_test::Expect;

mod cursor;
mod edit;
mod open;
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
    tmpfile.flush().unwrap();

    let (mut editor, ..) = zi::Editor::new(zi::Size::new(
        view_size.width,
        view_size.height + zi::Editor::BOTTOM_BAR_HEIGHT,
    ));
    let buf = editor.open_active(tmpfile.path()).expect("failed to open buffer");
    assert_eq!(editor.active_buffer().id(), buf);

    if content.ends_with('\n') {
        assert_eq!(editor.active_buffer().text().to_string(), content);
    } else {
        // zi appends a newline to the end of the buffer if there isn't one already
        assert_eq!(editor.active_buffer().text().to_string(), format!("{content}\n"));
    }
    editor
}

pub fn snapshot(editor: &zi::Editor, expect: Expect) {
    expect.assert_debug_eq(&editor.display_active());
}
