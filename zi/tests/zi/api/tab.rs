use super::*;

#[test]
fn tab_insert() {
    let mut editor = new("");
    editor.tab(zi::Active);
    editor.buffer(zi::Active).settings();
}
