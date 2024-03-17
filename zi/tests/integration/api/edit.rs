use expect_test::expect;

use crate::api::{new, snapshot};

#[test]
fn delete_char_backward() {
    let mut editor = new("");
    editor.set_mode(zi::Mode::Insert);
    editor.insert_char('a');
    editor.insert_char('b');
    editor.insert_char('c');

    // works on single line
    assert_eq!(editor.current_line(), "abc\n");
    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "ab\n");
    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "a\n");
    editor.insert_char('x');
    assert_eq!(editor.current_line(), "ax\n");
    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "a\n");
    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "\n");
    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "\n");

    // works on multiple lines
    editor.insert("abc\nd");
    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "\n");
    assert_eq!(editor.active_cursor(), (1, 0));
    editor.delete_char_backward();
    assert_eq!(editor.current_line(), "abc\n");
    assert_eq!(editor.active_cursor(), (0, 3));

    expect![[r#"
         1 abc|

    "#]]
    .assert_debug_eq(&editor.display_active());

    editor.input("<CR><ESC>oghi<ESC>kidef").unwrap();

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
    assert_eq!(editor.active_cursor(), (0, 0));
    editor.set_mode(zi::Mode::Insert);
    assert_eq!(editor.active_cursor(), (0, 0));

    assert_eq!(editor.active_cursor(), (0, 0));
    editor.insert_char('a');
    assert_eq!(editor.active_cursor(), (0, 1));
    editor.insert_char('b');
    assert_eq!(editor.active_cursor(), (0, 2));
    assert_eq!(editor.current_line(), "ab\n");

    editor.set_mode(zi::Mode::Normal);
    assert_eq!(editor.active_cursor(), (0, 1), "insert mode should move cursor left on exit");

    editor.set_active_cursor((1, 0));
    editor.set_mode(zi::Mode::Insert);
    editor.set_mode(zi::Mode::Normal);
    assert_eq!(editor.active_cursor(), (0, 0), "nowhere left to move");
}

#[test]
fn insert_into_readonly() -> zi::Result<()> {
    let mut editor = new("abc");
    let path = tempfile::NamedTempFile::new()?.into_temp_path();
    let buf = editor.open(path, zi::OpenFlags::READONLY | zi::OpenFlags::SET_ACTIVE_BUFFER)?;
    assert!(editor.buffer(buf).flags().contains(zi::BufferFlags::READONLY));

    assert!(editor.get_error().is_none());

    editor.insert("def");

    assert_eq!(editor.buffer(buf).text().to_string(), "");
    assert!(editor.get_error().is_some());
    Ok(())
}
