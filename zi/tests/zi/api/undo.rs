use super::new;
use crate::new_cx;

#[test]
fn undo_insertion() {
    let mut editor = new("x");
    editor.set_mode(zi::Mode::Insert);
    for c in ['a', 'b', 'c'] {
        editor.insert_char(zi::Active, c);
    }

    editor.set_mode(zi::Mode::Normal);
    assert_eq!(editor.cursor_line(), "abcx");

    editor.undo(zi::Active);
    assert_eq!(editor.cursor_line(), "x");

    editor.redo(zi::Active);
    assert_eq!(editor.cursor_line(), "abcx");
}

#[test]
fn undo_textobject_deletion() {
    let mut editor = new("abc");
    editor.input("dd").unwrap();
    assert_eq!(editor.cursor_line(), "");
    editor.undo(zi::Active);
    assert_eq!(editor.cursor_line(), "abc");
}

#[test]
fn undo_does_not_insert_extra_newlines() {
    let mut editor = new("a");
    editor.input("dwdw").unwrap();
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "");
    editor.undo(zi::Active);
    assert!(!editor.buffer(zi::Active).text().to_string().ends_with("\n\n"));
}

#[test]
fn undo_dwdwdd() {
    let mut editor = new("a");
    editor.input("dwdw").unwrap();
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "");
    editor.input("dd").unwrap();
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "");
}

#[tokio::test]
async fn undo_marks_buffer_dirty() {
    let cx = new_cx("abc").await;
    cx.with(|editor| {
        editor.buffer(zi::Active).flags();
    })
    .await;
}
