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

#[test]
fn undo_uncommited_changes() {
    let mut editor = new("");
    editor.set_mode(zi::Mode::Insert);
    editor.insert(zi::Active, "ab");
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "ab\n");

    // These changes usually won't be committed to the undo history until we exit insert mode.
    // However, calling undo should commit them, then undo them.
    editor.undo(zi::Active);
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "");

    editor.redo(zi::Active);
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "ab\n");
}

#[tokio::test]
async fn undo_marks_buffer_dirty() -> zi::Result<()> {
    let is_dirty =
        |editor: &zi::Editor| editor.buffer(zi::Active).flags().contains(zi::BufferFlags::DIRTY);

    let cx = new_cx("").await;

    cx.open("test", zi::OpenFlags::ACTIVE).await?;

    cx.with(move |editor| {
        assert_eq!(editor.buffer(zi::Active).text().to_string(), "test");
        assert!(!is_dirty(editor));
        editor.insert_char(zi::Active, 'a');
        assert!(is_dirty(editor));
        editor.save(zi::Active, zi::SaveFlags::empty())
    })
    .await
    .await?;

    cx.with(move |editor| {
        assert!(!is_dirty(editor));
        assert_eq!(editor.buffer(zi::Active).text().to_string(), "atest\n");
        editor.undo(zi::Active);
        assert_eq!(editor.buffer(zi::Active).text().to_string(), "test");
        assert!(is_dirty(editor));
    })
    .await;

    Ok(())
}
