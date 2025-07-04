use crate::new;

#[tokio::test]
async fn picker() {
    let cx = new("").await;
    cx.with(|editor| {
        assert_eq!(editor.views().count(), 1);
        let current_buf = editor.buffer(zi::Active).id();

        editor.open_file_picker(".");
        assert_ne!(
            editor.buffer(zi::Active).id(),
            current_buf,
            "view should be focused on new buffer of picker"
        );

        editor.close_view(zi::Active);
        assert_eq!(
            editor.buffer(zi::Active).id(),
            current_buf,
            "view should be focused on original buffer after closing picker"
        );

        assert_eq!(editor.views().count(), 1, "the picker views should be gone");

        editor.open_file_picker(".");
        assert_eq!(
            editor.views().count(),
            4,
            "the file picker consists of 3 views (picker, results, and preview)"
        );

        editor.open_file_picker(".");
        assert_eq!(
            editor.views().count(),
            4,
            "opening another picker should not create new set of views"
        );
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn picker_input() {
    let cx = new("").await;
    cx.with(|editor| {
        editor.open_jump_list(zi::Active);
        editor.input("abc").unwrap();
        assert_eq!(editor.text(zi::Active), "abc");
        assert_eq!(editor.cursor(zi::Active), (0, 3));

        editor.input("<BS>").unwrap();
        assert_eq!(editor.text(zi::Active), "ab");
        assert_eq!(editor.cursor(zi::Active), (0, 2));
    })
    .await;
    cx.cleanup().await;
}
