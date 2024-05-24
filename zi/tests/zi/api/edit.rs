use expect_test::expect;

use crate::api::{new, snapshot};

#[tokio::test]
async fn delete_char_backward() {
    let mut editor = new("").await;
    editor.set_mode(zi::Mode::Insert);

    // ensure that multi-byte characters are handled correctly
    let c = '\u{100000}';
    editor.insert_char(zi::Active, c);
    assert_eq!(editor.cursor(zi::Active), (0, c.len_utf8()));
    editor.delete_char(zi::Active);
    assert_eq!(editor.cursor_line(), "");

    editor.insert_char(zi::Active, 'a');
    editor.insert_char(zi::Active, 'b');
    editor.insert_char(zi::Active, 'c');

    // works on single line
    assert_eq!(editor.cursor_line(), "abc");
    assert_eq!(editor.cursor(zi::Active), (0, 3));

    editor.delete_char(zi::Active);
    assert_eq!(editor.cursor_line(), "ab");
    assert_eq!(editor.cursor(zi::Active), (0, 2));

    editor.delete_char(zi::Active);
    assert_eq!(editor.cursor_line(), "a");
    assert_eq!(editor.cursor(zi::Active), (0, 1));

    editor.insert_char(zi::Active, 'x');
    assert_eq!(editor.cursor_line(), "ax");
    assert_eq!(editor.cursor(zi::Active), (0, 2));

    editor.delete_char(zi::Active);
    assert_eq!(editor.cursor_line(), "a");
    assert_eq!(editor.cursor(zi::Active), (0, 1));

    editor.delete_char(zi::Active);
    assert_eq!(editor.cursor_line(), "");
    assert_eq!(editor.cursor(zi::Active), (0, 0));

    editor.delete_char(zi::Active);
    assert_eq!(editor.cursor_line(), "");
    assert_eq!(editor.cursor(zi::Active), (0, 0));

    // works on multiple lines
    editor.insert(zi::Active, "abc\nd");
    assert_eq!(editor.cursor_line(), "d");
    editor.delete_char(zi::Active);
    assert_eq!(editor.cursor_line(), "");
    assert_eq!(editor.cursor(zi::Active), (1, 0));
    editor.delete_char(zi::Active);
    assert_eq!(editor.cursor_line(), "abc");
    assert_eq!(editor.cursor(zi::Active), (0, 3));

    snapshot(
        &editor,
        expect![[r#"
             1 abc|

        "#]],
    );

    editor.input("<CR><ESC>oghi<ESC>kidef").unwrap();

    assert_eq!(editor.mode(), zi::Mode::Insert);

    snapshot(
        &editor,
        expect![[r#"
             1 abc
             2 de|
             3 ghi

        "#]],
    );

    editor.input("<BS>").unwrap();

    snapshot(
        &editor,
        expect![[r#"
             1 abc
             2 d|
             3 ghi

        "#]],
    );

    editor.input("<BS><BS>").unwrap();

    snapshot(
        &editor,
        expect![[r#"
             1 abc|
             2 ghi

        "#]],
    );
}

#[tokio::test]
async fn insert_char() {
    let mut editor = new("").await;
    assert_eq!(editor.cursor(zi::Active), (0, 0));
    editor.set_mode(zi::Mode::Insert);
    assert_eq!(editor.cursor(zi::Active), (0, 0));

    assert_eq!(editor.cursor(zi::Active), (0, 0));
    editor.insert_char(zi::Active, 'a');
    assert_eq!(editor.cursor(zi::Active), (0, 1));
    editor.insert_char(zi::Active, 'b');
    assert_eq!(editor.cursor(zi::Active), (0, 2));
    assert_eq!(editor.cursor_line(), "ab");

    editor.set_mode(zi::Mode::Normal);
    assert_eq!(editor.cursor(zi::Active), (0, 1), "insert mode should move cursor left on exit");

    editor.set_cursor(zi::Active, (1, 0));
    editor.set_mode(zi::Mode::Insert);
    editor.set_mode(zi::Mode::Normal);
    assert_eq!(editor.cursor(zi::Active), (0, 0), "nowhere left to move");
}

#[tokio::test]
async fn insert_into_readonly() -> zi::Result<()> {
    let mut editor = new("abc").await;
    let path = tempfile::NamedTempFile::new()?.into_temp_path();
    let buf = editor.open(path, zi::OpenFlags::READONLY | zi::OpenFlags::ACTIVE)?.await?;
    assert!(editor.buffer(buf).flags().contains(zi::BufferFlags::READONLY));

    assert!(editor.get_error().is_none());

    editor.insert(zi::Active, "def");

    assert_eq!(editor.buffer(buf).text().to_string(), "");
    assert!(editor.get_error().is_some());
    Ok(())
}

#[tokio::test]
async fn enter_normal_mode_on_last_line() -> zi::Result<()> {
    let mut editor = new("").await;
    editor.input("iabc<ESC>o").unwrap();
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "abc\n\n");
    assert_eq!(editor.cursor(zi::Active), (1, 0));
    editor.input("<ESC>").unwrap();
    assert_eq!(editor.cursor(zi::Active), (1, 0));
    Ok(())
}

#[tokio::test]
async fn delete_last_line() {
    let mut editor = new("").await;
    editor.input("i<CR><CR><ESC>").unwrap();
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "\n\n\n");
    editor.input("jjdd").unwrap();
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "\n\n");
    assert_eq!(editor.cursor(zi::Active), (1, 0));
}
