use expect_test::expect;

use super::run;

#[tokio::test]
async fn line_number_width_grows_and_shrinks_as_required() {
    run(
        zi::Size::new(51, 8),
        &(1..13).map(|n| n.to_string()).collect::<Vec<_>>().join("\n"),
        |editor, mut snapshot| {
            editor.view(zi::Active).config().line_number_width.write(0);

            // the text should be aligned to the largest line number
            // i.e. all lines should be indented by 3 spaces
            snapshot(
                editor,
                expect![[r#"
            "  7 7                                              "
            "  8 8                                              "
            "  9 9                                              "
            " 10 10                                             "
            " 11 11                                             "
            " 12 12|                                            "
            "scratch:12:2                                       "
            "-- INSERT --                                       "
        "#]],
            );

            editor.view(zi::Active).config().line_number_style.write(zi::LineNumberStyle::Relative);

            snapshot(
                editor,
                expect![[r#"
            "  5 7                                              "
            "  4 8                                              "
            "  3 9                                              "
            "  2 10                                             "
            "  1 11                                             "
            " 12 12|                                            "
            "scratch:12:2                                       "
            "-- INSERT --                                       "
        "#]],
            );

            editor.move_cursor(zi::Active, zi::Direction::Up, 3);

            snapshot(
                editor,
                expect![[r#"
            " 2 7                                               "
            " 1 8                                               "
            " 9 9|                                              "
            " 1 10                                              "
            " 2 11                                              "
            " 3 12                                              "
            "scratch:9:1                                        "
            "-- INSERT --                                       "
        "#]],
            );
        },
    )
    .await;
}

#[tokio::test]
async fn no_line_number() {
    run(zi::Size::new(51, 8), "a\nb\nc", |editor, mut snapshot| {
        editor.view(zi::Active).config().line_number_style.write(zi::LineNumberStyle::None);

        snapshot(
            editor,
            expect![[r#"
            "  a                                                "
            "  b                                                "
            "  c|                                               "
            "                                                   "
            "                                                   "
            "                                                   "
            "scratch:3:1                                        "
            "-- INSERT --                                       "
        "#]],
        );
    })
    .await
}

#[tokio::test]
async fn relative_line_number() {
    run(
        zi::Size::new(51, 8),
        &(1..13).map(|n| n.to_string()).collect::<Vec<_>>().join("\n"),
        |editor, mut snapshot| {
            editor.move_cursor(zi::Active, zi::Direction::Up, 3);

            editor.view(zi::Active).config().line_number_style.write(zi::LineNumberStyle::Relative);

            snapshot(
                editor,
                expect![[r#"
            "   2 7                                             "
            "   1 8                                             "
            "   9 9|                                            "
            "   1 10                                            "
            "   2 11                                            "
            "   3 12                                            "
            "scratch:9:1                                        "
            "-- INSERT --                                       "
        "#]],
            );

            editor.move_cursor(zi::Active, zi::Direction::Down, 1);

            snapshot(
                editor,
                expect![[r#"
            "   3 7                                             "
            "   2 8                                             "
            "   1 9                                             "
            "  10 10|                                           "
            "   1 11                                            "
            "   2 12                                            "
            "scratch:10:2                                       "
            "-- INSERT --                                       "
        "#]],
            );
        },
    )
    .await
}
