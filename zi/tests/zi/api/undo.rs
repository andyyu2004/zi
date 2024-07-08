use crate::new;

#[tokio::test]
async fn undo_insertion() {
    let cx = new("x").await;
    cx.with(|editor| {
        editor.set_mode(zi::Mode::Insert);
        for c in ['a', 'b', 'c'] {
            editor.insert_char(zi::Active, c).unwrap();
        }

        editor.set_mode(zi::Mode::Normal);
        assert_eq!(editor.cursor_line(), "abcx");

        editor.undo(zi::Active).unwrap();
        assert_eq!(editor.cursor_line(), "x");

        editor.redo(zi::Active).unwrap();
        assert_eq!(editor.cursor_line(), "abcx");
    })
    .await;

    cx.cleanup().await;
}

#[tokio::test]
async fn undo_textobject_deletion() {
    let cx = new("abc").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("dw").unwrap();
        assert_eq!(editor.cursor_line(), "");
        editor.undo(zi::Active).unwrap();
        assert_eq!(editor.cursor_line(), "abc");
    })
    .await;

    cx.cleanup().await;
}

#[tokio::test]
async fn undo_does_not_insert_extra_newlines() {
    let cx = new("a").await;
    cx.with(|editor| {
        editor.input("dwdw").unwrap();
        assert_eq!(editor.text(zi::Active), "");
        editor.undo(zi::Active).unwrap();
        assert!(!editor.text(zi::Active).to_string().ends_with("\n\n"));
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn undo_dwdwdd() {
    let cx = new("a").await;
    cx.with(|editor| {
        editor.input("dwdw").unwrap();
        assert_eq!(editor.text(zi::Active), "");
        editor.input("dd").unwrap();
        assert_eq!(editor.text(zi::Active), "");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn undo_uncommitted_changes() {
    let cx = new("").await;
    cx.with(|editor| {
        editor.set_mode(zi::Mode::Insert);
        editor.insert(zi::Active, "ab").unwrap();
        assert_eq!(editor.text(zi::Active), "ab\n");

        // These changes usually won't be committed to the undo history until we exit insert mode.
        // However, calling undo should commit them, then undo them.
        editor.undo(zi::Active).unwrap();
        assert_eq!(editor.text(zi::Active), "");

        editor.redo(zi::Active).unwrap();
        assert_eq!(editor.text(zi::Active), "ab\n");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn undo_marks_buffer_dirty() -> zi::Result<()> {
    let is_dirty =
        |editor: &zi::Editor| editor.buffer(zi::Active).flags().contains(zi::BufferFlags::DIRTY);

    let cx = new("").await;

    cx.open_tmp("test", zi::OpenFlags::empty()).await?;

    cx.with(move |editor| {
        editor.set_cursor(zi::Active, (0, 0));
        assert_eq!(editor.text(zi::Active), "test");
        assert!(!is_dirty(editor));
        editor.insert_char(zi::Active, 'a').unwrap();
        assert!(is_dirty(editor));
        editor.save(zi::Active, zi::SaveFlags::empty())
    })
    .await
    .await?;

    cx.with(move |editor| {
        assert!(!is_dirty(editor));
        assert_eq!(editor.text(zi::Active), "atest\n");
        editor.undo(zi::Active).unwrap();
        assert_eq!(editor.text(zi::Active), "test");
        assert!(is_dirty(editor));
    })
    .await;

    cx.cleanup().await;

    Ok(())
}
