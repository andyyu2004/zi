use expect_test::expect;

use crate::render::new_cx;

#[tokio::test]
async fn insert_newline_in_blank_buffer() {
    let cx = new_cx(zi::Size::new(10, 5), "").await;
    cx.snapshot(expect![[r#"
        "   1 |    "
        "          "
        "          "
        "scratch:1:"
        "-- INSERT "
    "#]])
        .await;

    cx.with(|editor| editor.insert_char(zi::Active, '\n')).await;

    cx.snapshot(expect![[r#"
        "   1      "
        "   2 |    "
        "          "
        "scratch:2:"
        "-- INSERT "
    "#]])
        .await;
}
