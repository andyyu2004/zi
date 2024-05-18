use expect_test::expect;

use crate::new_cx_with_size;

#[tokio::test]
async fn insert_newline_in_blank_buffer() {
    let cx = new_cx_with_size(zi::Size::new(10, 5), "").await;
    cx.snapshot(expect![[r#"
        "   1 |    "
        "          "
        "          "
        "buffer://s"
        "-- INSERT "
    "#]])
        .await;

    cx.with(|editor| editor.insert_char(zi::Active, '\n')).await;

    cx.snapshot(expect![[r#"
        "   1      "
        "   2 |    "
        "          "
        "buffer://s"
        "-- INSERT "
    "#]])
        .await;
}
