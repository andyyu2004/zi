use expect_test::expect;

use super::run;

#[tokio::test]
async fn insert_newline_in_blank_buffer() {
    run(zi::Size::new(10, 5), "", |editor, mut snapshot| {
        snapshot(
            editor,
            expect![[r#"
            "   1 |    "
            "          "
            "          "
            "scratch:1:"
            "-- INSERT "
        "#]],
        );

        editor.insert_char_at_cursor('\n');
        snapshot(
            editor,
            expect![[r#"
            "   1      "
            "   2 |    "
            "          "
            "scratch:2:"
            "-- INSERT "
        "#]],
        );
    })
    .await;
}
