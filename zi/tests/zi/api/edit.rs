use expect_test::expect;

use crate::new;

#[tokio::test]
async fn delete_char_backward() {
    let cx = new("").await;
    cx.with(|editor| {
        editor.set_mode(zi::Mode::Insert);

        // ensure that multi-byte characters are handled correctly
        let c = '\u{100000}';
        editor.insert_char(zi::Active, c).unwrap();
        assert_eq!(editor.cursor(zi::Active), (0, c.len_utf8()));
        editor.delete_char(zi::Active);
        assert_eq!(editor.cursor_line(), "");

        editor.insert_char(zi::Active, 'a').unwrap();
        editor.insert_char(zi::Active, 'b').unwrap();
        editor.insert_char(zi::Active, 'c').unwrap();

        // works on single line
        assert_eq!(editor.cursor_line(), "abc");
        assert_eq!(editor.cursor(zi::Active), (0, 3));

        editor.delete_char(zi::Active);
        assert_eq!(editor.cursor_line(), "ab");
        assert_eq!(editor.cursor(zi::Active), (0, 2));

        editor.delete_char(zi::Active);
        assert_eq!(editor.cursor_line(), "a");
        assert_eq!(editor.cursor(zi::Active), (0, 1));

        editor.insert_char(zi::Active, 'x').unwrap();
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
        editor.insert(zi::Active, "abc\nd").unwrap();
        assert_eq!(editor.cursor_line(), "d");
        editor.delete_char(zi::Active);
        assert_eq!(editor.cursor_line(), "");
        assert_eq!(editor.cursor(zi::Active), (1, 0));
        editor.delete_char(zi::Active);
        assert_eq!(editor.cursor_line(), "abc");
        assert_eq!(editor.cursor(zi::Active), (0, 3));
    })
    .await;

    cx.snapshot(expect![[r#"
        "   1 abc|                                                                       "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "buffer://scratch:1:3                                                            "
        "-- INSERT --                                                                    "
    "#]])
        .await;

    cx.with(|editor| {
        editor.input("<CR><ESC>oghi<ESC>kidef").unwrap();
        assert_eq!(editor.mode(), zi::Mode::Insert);
    })
    .await;

    cx.snapshot(expect![[r#"
        "   1 abc                                                                        "
        "   2 de|                                                                        "
        "   3 ghi                                                                        "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "buffer://scratch:2:2                                                            "
        "-- INSERT --                                                                    "
    "#]])
        .await;

    cx.with(|editor| {
        editor.input("<BS>").unwrap();
    })
    .await;

    cx.snapshot(expect![[r#"
        "   1 abc                                                                        "
        "   2 d|                                                                         "
        "   3 ghi                                                                        "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "buffer://scratch:2:1                                                            "
        "-- INSERT --                                                                    "
    "#]])
        .await;

    cx.with(|editor| editor.input("<BS>").unwrap()).await;

    cx.snapshot(expect![[r#"
        "   1 abc                                                                        "
        "   2 |                                                                          "
        "   3 ghi                                                                        "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "                                                                                "
        "buffer://scratch:2:0                                                            "
        "-- INSERT --                                                                    "
    "#]])
        .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn insert_char() {
    let cx = new("").await;
    cx.with(|editor| {
        assert_eq!(editor.cursor(zi::Active), (0, 0));
        editor.set_mode(zi::Mode::Insert);
        assert_eq!(editor.cursor(zi::Active), (0, 0));

        assert_eq!(editor.cursor(zi::Active), (0, 0));
        editor.insert_char(zi::Active, 'a').unwrap();
        assert_eq!(editor.cursor(zi::Active), (0, 1));
        editor.insert_char(zi::Active, 'b').unwrap();
        assert_eq!(editor.cursor(zi::Active), (0, 2));
        assert_eq!(editor.cursor_line(), "ab");

        editor.set_mode(zi::Mode::Normal);
        assert_eq!(
            editor.cursor(zi::Active),
            (0, 1),
            "insert mode should move cursor left on exit"
        );

        editor.set_cursor(zi::Active, (1, 0));
        editor.set_mode(zi::Mode::Insert);
        editor.set_mode(zi::Mode::Normal);
        assert_eq!(editor.cursor(zi::Active), (0, 0), "nowhere left to move");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn insert_into_readonly() -> zi::Result<()> {
    let cx = new("abc").await;
    let file = tempfile::NamedTempFile::new()?;
    let path = file.path().to_path_buf();

    let buf = cx.open(&path, zi::OpenFlags::READONLY).await?;

    cx.with(move |editor| {
        assert!(editor.buffer(buf).flags().contains(zi::BufferFlags::READONLY));

        assert!(matches!(editor.insert(zi::Active, "def"), Err(zi::EditError::Readonly)));

        assert_eq!(editor.buffer(buf).text().to_string(), "");
    })
    .await;
    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn enter_normal_mode_on_last_line() {
    let cx = new("").await;
    cx.with(|editor| {
        editor.input("iabc<ESC>o").unwrap();
        assert_eq!(editor.buffer(zi::Active).text().to_string(), "abc\n\n");
        assert_eq!(editor.cursor(zi::Active), (1, 0));
        editor.input("<ESC>").unwrap();
        assert_eq!(editor.cursor(zi::Active), (1, 0));
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn delete_last_line() {
    let cx = new("").await;
    cx.with(|editor| {
        editor.input("i<CR><CR><ESC>").unwrap();
        assert_eq!(editor.buffer(zi::Active).text().to_string(), "\n\n\n");
        editor.input("jjdd").unwrap();
        assert_eq!(editor.buffer(zi::Active).text().to_string(), "\n\n");
        assert_eq!(editor.cursor(zi::Active), (1, 0));
    })
    .await;
    cx.cleanup().await;
}
