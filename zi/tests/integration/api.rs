use std::io::Write;
mod cursor;
mod insert;
mod scroll;

/// Create a new editor with a buffer open with the given content.
pub fn new(content: &str) -> zi::Editor {
    let mut tmpfile = tempfile::NamedTempFile::new().expect("could not create tempfile");
    tmpfile.write_all(content.as_bytes()).expect("could not write to tempfile");
    let (mut editor, _) = zi::Editor::new(zi::Size::new(80, 24));
    editor.open(tmpfile.path()).expect("failed to open buffer");
    editor
}
