use zi::buffer::IndentSettings;

use super::*;

#[test]
fn tab_insert() {
    let mut editor = new("");
    assert_eq!(*editor.buffer(zi::Active).settings().indent.read(), IndentSettings::Spaces(4));

    editor.set_mode(zi::Mode::Insert);
    assert_eq!(editor.cursor_line(), "");
    editor.tab(zi::Active);
    assert_eq!(editor.cursor_line(), "    ");
    editor.tab(zi::Active);
    assert_eq!(editor.cursor_line(), "        ");
}
