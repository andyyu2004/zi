use super::*;

#[tokio::test]
async fn file_picker() {
    run(zi::Size::new(51, 10), "", |editor, mut snapshot| {
        editor.open_file_picker(".");

        snapshot(
            editor,
            expect![[r#"
                "   1                                               "
                "                                                   "
                "                                                   "
                "                                                   "
                "  |                                                "
                "                                                   "
                "                                                   "
                "                                                   "
                "picker:1:0                                         "
                "-- INSERT --                                       "
            "#]],
        );
    })
    .await;
}
