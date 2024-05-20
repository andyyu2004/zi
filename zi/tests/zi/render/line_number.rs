use expect_test::expect;

use crate::new_cx_with_size;

#[tokio::test]
async fn line_number_width_grows_and_shrinks_as_required() {
    let text = (1..13).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
    let cx = new_cx_with_size(zi::Size::new(51, 8), &text).await;

    cx.with(|editor| editor.view(zi::Active).settings().line_number_width.write(0)).await;
    cx.snapshot(expect![[r#"
        "  7 7                                              "
        "  8 8                                              "
        "  9 9                                              "
        " 10 10                                             "
        " 11 11                                             "
        " 12 1|                                             "
        "buffer://scratch:12:1                              "
        "                                                   "
    "#]])
        .await;

    cx.with(|editor| {
        editor.view(zi::Active).settings().line_number_style.write(zi::LineNumberStyle::Relative);
        editor.move_cursor(zi::Active, zi::Direction::Up, 3);
    })
    .await;

    cx.snapshot(expect![[r#"
        " 2 7                                               "
        " 1 8                                               "
        " 9 |                                               "
        " 1 10                                              "
        " 2 11                                              "
        " 3 12                                              "
        "buffer://scratch:9:0                               "
        "                                                   "
    "#]])
        .await;

    cx.cleanup().await;
}

#[tokio::test]
async fn no_line_number() {
    let cx = new_cx_with_size(zi::Size::new(51, 8), "a\nb\nc").await;

    cx.with(|editor| {
        editor.view(zi::Active).settings().line_number_style.write(zi::LineNumberStyle::None)
    })
    .await;

    cx.snapshot(expect![[r#"
        "  a                                                "
        "  b                                                "
        "  |                                                "
        "                                                   "
        "                                                   "
        "                                                   "
        "buffer://scratch:3:0                               "
        "                                                   "
    "#]])
        .await;

    cx.cleanup().await;
}

#[tokio::test]
async fn relative_line_number() {
    let text = (1..13).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
    let cx = new_cx_with_size(zi::Size::new(51, 8), &text).await;

    cx.with(|editor| {
        editor.move_cursor(zi::Active, zi::Direction::Up, 3);
        editor.view(zi::Active).settings().line_number_style.write(zi::LineNumberStyle::Relative)
    })
    .await;

    cx.snapshot(expect![[r#"
        "   2 7                                             "
        "   1 8                                             "
        "   9 |                                             "
        "   1 10                                            "
        "   2 11                                            "
        "   3 12                                            "
        "buffer://scratch:9:0                               "
        "                                                   "
    "#]])
        .await;

    cx.with(|editor| editor.move_cursor(zi::Active, zi::Direction::Down, 1)).await;

    cx.snapshot(expect![[r#"
        "   3 7                                             "
        "   2 8                                             "
        "   1 9                                             "
        "  10 1|                                            "
        "   1 11                                            "
        "   2 12                                            "
        "buffer://scratch:10:1                              "
        "                                                   "
    "#]])
        .await;

    cx.cleanup().await;
}
