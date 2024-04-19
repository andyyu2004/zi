use expect_test::expect;

use crate::api::{new, snapshot};

#[test]
fn delete_char_backward() {
    let mut editor = new("");
    editor.set_mode(zi::Mode::Insert);

    // ensure that multi-byte characters are handled correctly
    let c = '\u{100000}';
    editor.insert_char_at_cursor(c);
    assert_eq!(editor.cursor(zi::Active), (0, c.len_utf8() as u32));
    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "");

    editor.insert_char_at_cursor('a');
    editor.insert_char_at_cursor('b');
    editor.insert_char_at_cursor('c');

    // works on single line
    assert_eq!(editor.current_line(), "abc");
    assert_eq!(editor.cursor(zi::Active), (0, 3));

    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "ab");
    assert_eq!(editor.cursor(zi::Active), (0, 2));

    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "a");
    assert_eq!(editor.cursor(zi::Active), (0, 1));

    editor.insert_char_at_cursor('x');
    assert_eq!(editor.current_line(), "ax");
    assert_eq!(editor.cursor(zi::Active), (0, 2));

    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "a");
    assert_eq!(editor.cursor(zi::Active), (0, 1));

    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "");
    assert_eq!(editor.cursor(zi::Active), (0, 0));

    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "");
    assert_eq!(editor.cursor(zi::Active), (0, 0));

    // works on multiple lines
    editor.insert_at_cursor("abc\nd");
    assert_eq!(editor.current_line(), "d");
    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "");
    assert_eq!(editor.cursor(zi::Active), (1, 0));
    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "abc");
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

#[test]
fn insert_char() {
    let mut editor = new("");
    assert_eq!(editor.cursor(zi::Active), (0, 0));
    editor.set_mode(zi::Mode::Insert);
    assert_eq!(editor.cursor(zi::Active), (0, 0));

    assert_eq!(editor.cursor(zi::Active), (0, 0));
    editor.insert_char_at_cursor('a');
    assert_eq!(editor.cursor(zi::Active), (0, 1));
    editor.insert_char_at_cursor('b');
    assert_eq!(editor.cursor(zi::Active), (0, 2));
    assert_eq!(editor.current_line(), "ab");

    editor.set_mode(zi::Mode::Normal);
    assert_eq!(editor.cursor(zi::Active), (0, 1), "insert mode should move cursor left on exit");

    editor.set_cursor(zi::Active, (1, 0));
    editor.set_mode(zi::Mode::Insert);
    editor.set_mode(zi::Mode::Normal);
    assert_eq!(editor.cursor(zi::Active), (0, 0), "nowhere left to move");
}

#[test]
fn insert_into_readonly() -> zi::Result<()> {
    let mut editor = new("abc");
    let path = tempfile::NamedTempFile::new()?.into_temp_path();
    let buf = editor.open(path, zi::OpenFlags::READONLY | zi::OpenFlags::SET_ACTIVE_BUFFER)?;
    assert!(editor.buffer(buf).flags().contains(zi::BufferFlags::READONLY));

    assert!(editor.get_error().is_none());

    editor.insert_at_cursor("def");

    assert_eq!(editor.buffer(buf).text().to_string(), "");
    assert!(editor.get_error().is_some());
    Ok(())
}

#[test]
fn enter_normal_mode_on_last_line() -> zi::Result<()> {
    let mut editor = new("");
    editor.input("iabc<ESC>o").unwrap();
    assert_eq!(editor.buffer(zi::Active).text().to_string(), "abc\n\n");
    assert_eq!(editor.cursor(zi::Active), (1, 0));
    editor.input("<ESC>").unwrap();
    assert_eq!(editor.cursor(zi::Active), (1, 0));
    Ok(())
}
