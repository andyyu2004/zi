use expect_test::expect;

use crate::new;

#[tokio::test]
async fn insert_newline_in_blank_buffer() {
    let cx = new("").with_size((10, 5)).await;
    cx.snapshot(expect![[r#"
        "   1 |    "
        "          "
        "          "
        "buffer://s"
        "          "
    "#]])
        .await;

    cx.with(|editor| editor.insert_char(zi::Active, '\n')).await;

    cx.snapshot(expect![[r#"
        "   1      "
        "   2 |    "
        "          "
        "buffer://s"
        "          "
    "#]])
        .await;

    cx.cleanup().await;
}
