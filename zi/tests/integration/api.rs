use std::io::Write;

use ratatui::backend::TestBackend;
use ratatui::Terminal;
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
    editor.open(tmpfile.path()).expect("failed to open buffer");
    editor
}

pub fn new_with_snapshot(
    size: zi::Size,
    content: &str,
) -> (zi::Editor, impl FnMut(&mut zi::Editor, expect_test::Expect)) {
    let (mut editor, ..) = zi::Editor::new(size);
    for c in content.chars() {
        editor.insert_char(c);
    }

    let mut term = Terminal::new(TestBackend::new(size.width, size.height)).unwrap();
    (editor, move |editor, expect| {
        term.draw(|f| editor.render(f.size(), f.buffer_mut())).unwrap();
        expect.assert_eq(&term.backend().to_string())
    })
}
