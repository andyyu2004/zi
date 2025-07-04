use std::{iter, ops};

use zi::Active;

use crate::new;

#[track_caller]
fn next(editor: &mut zi::Editor) -> ops::Range<usize> {
    editor.goto_next_match().unwrap().range()
}

#[track_caller]
fn prev(editor: &mut zi::Editor) -> ops::Range<usize> {
    editor.goto_prev_match().unwrap().range()
}

#[tokio::test]
async fn search_switch_buffers() {
    let cx = new("abc\nabc").await;
    cx.with(|editor| {
        assert!(editor.search("abc").map(|m| m.range()).eq([0..3, 4..7]));

        let buf = editor.create_readonly_buffer("path", "abc".as_bytes());
        editor.set_buffer(Active, buf);

        // switching buffers should recompute the search
        assert_eq!(editor.cursor_line(), "abc");
        assert!(editor.matches().map(|m| m.range()).eq(iter::once(0..3)));
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn search_cursor() {
    let cx = new(r#"a
a
a
a"#)
    .await;
    cx.with(|editor| {
        editor.set_cursor(Active, (0, 0));
        editor.input("jj").unwrap();
        assert!(editor.search("a").map(|m| m.range()).eq([0..1, 2..3, 4..5, 6..7]));
        assert_eq!(next(editor), 6..7);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn search_smoke() {
    // TODO what if we switch active buffers and a match is out of range etc
    let cx = new(r#"abc
def
abc
"#)
    .await;

    cx.with(|editor| {
        assert_eq!(editor.matches().len(), 0);

        assert!(editor.search("abc").map(|m| m.range()).eq([0..3, 8..11]));
        editor.input("<CR>").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Normal);

        assert_eq!(next(editor), 8..11);
        assert_eq!(next(editor), 0..3, "wraps around");
        assert_eq!(next(editor), 8..11);

        // moving should disable hl search and the next `n` should keep the current match
        editor.input("w").unwrap();
        assert_eq!(next(editor), 8..11);

        editor.input("w").unwrap();
        assert_eq!(prev(editor), 8..11);
        assert_eq!(prev(editor), 0..3);
        assert_eq!(prev(editor), 8..11);

        assert!(editor.search("no match for this").next().is_none());
        assert!(editor.goto_next_match().is_none());
        assert!(editor.goto_next_match().is_none());
    })
    .await;
    cx.cleanup().await;
}
