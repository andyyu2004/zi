use expect_test::expect;

use super::new_with_snapshot;

#[test]
fn line_number_width_grows_as_required() {
    let (mut editor, mut snapshot) = new_with_snapshot(
        zi::Size::new(51, 8),
        &(1..13).map(|n| n.to_string()).collect::<Vec<_>>().join("\n"),
    );

    editor.view(zi::Active).config().line_number_width.write(0);

    snapshot(
        &mut editor,
        expect![[r#"
            "7 7                                                "
            "8 8                                                "
            "9 9                                                "
            "10 10                                              "
            "11 11                                              "
            "12|12                                              "
            "scratch:12:2                                       "
            "-- INSERT --                                       "
        "#]],
    );
}

#[test]
fn off_line_number() {
    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(51, 8), "a\nb\nc");

    editor.view(zi::Active).config().line_number_style.write(zi::LineNumberStyle::None);

    snapshot(
        &mut editor,
        expect![[r#"
            " a                                                 "
            " b                                                 "
            " c   |                                             "
            "                                                   "
            "                                                   "
            "                                                   "
            "scratch:3:1                                        "
            "-- INSERT --                                       "
        "#]],
    );
}

#[test]
fn relative_line_number() {
    let (mut editor, mut snapshot) = new_with_snapshot(
        zi::Size::new(51, 8),
        &(1..13).map(|n| n.to_string()).collect::<Vec<_>>().join("\n"),
    );

    editor.move_cursor(zi::Active, zi::Direction::Up, 3);

    editor.view(zi::Active).config().line_number_style.write(zi::LineNumberStyle::Relative);

    snapshot(
        &mut editor,
        expect![[r#"
            "  2 7                                              "
            "  1 8                                              "
            "  9 9|                                             "
            "  1 10                                             "
            "  2 11                                             "
            "  3 12                                             "
            "scratch:9:1                                        "
            "-- INSERT --                                       "
        "#]],
    );

    editor.move_cursor(zi::Active, zi::Direction::Down, 1);

    snapshot(
        &mut editor,
        expect![[r#"
            "  3 7                                              "
            "  2 8                                              "
            "  1 9                                              "
            " 10 10|                                            "
            "  1 11                                             "
            "  2 12                                             "
            "scratch:10:2                                       "
            "-- INSERT --                                       "
        "#]],
    );
}
