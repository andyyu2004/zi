use std::io;

use expect_test::expect;

use super::new_with_snapshot;

#[test]
fn test_splits_have_independent_scroll() -> io::Result<()> {
    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(51, 8), "1\n2\n3\n");

    editor.split(zi::Direction::Right);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 1                       1 1                   "
            "   2 2                       2 2                   "
            "   3 3                       3 3                   "
            "   4                         4 |                   "
            "                                                   "
            "                                                   "
            "scratch:4:0                                        "
            "-- INSERT --                                       "
        "#]],
    );

    editor.scroll_active_view(zi::Direction::Down, 1);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 1                       2 2                   "
            "   2 2                       3 3                   "
            "   3 3                       4 |                   "
            "   4                                               "
            "                                                   "
            "                                                   "
            "scratch:4:0                                        "
            "-- INSERT --                                       "
        "#]],
    );

    Ok(())
}

#[test]
fn split() -> io::Result<()> {
    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(50, 8), "abc");

    editor.split(zi::Direction::Down);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc                                          "
            "                                                  "
            "                                                  "
            "   1 abc|                                         "
            "                                                  "
            "                                                  "
            "scratch:1:3                                       "
            "-- INSERT --                                      "
        "#]],
    );

    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(50, 8), "abc");

    editor.split(zi::Direction::Right);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc                    1 abc|                "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "scratch:1:3                                       "
            "-- INSERT --                                      "
        "#]],
    );

    editor.split(zi::Direction::Right);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc            1 abc           1 abc|        "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "scratch:1:3                                       "
            "-- INSERT --                                      "
        "#]],
    );

    editor.split(zi::Direction::Down);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc            1 abc           1 abc         "
            "                                                  "
            "                                                  "
            "                                    1 abc|        "
            "                                                  "
            "                                                  "
            "scratch:1:3                                       "
            "-- INSERT --                                      "
        "#]],
    );

    editor.split(zi::Direction::Left);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc            1 abc           1 abc         "
            "                                                  "
            "                                                  "
            "                                    1 abc|   1 abc"
            "                                                  "
            "                                                  "
            "scratch:1:3                                       "
            "-- INSERT --                                      "
        "#]],
    );

    editor.split(zi::Direction::Up);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc            1 abc           1 abc         "
            "                                                  "
            "                                                  "
            "                                    1 abc|   1 abc"
            "                                                  "
            "                                    1 abc         "
            "scratch:1:3                                       "
            "-- INSERT --                                      "
        "#]],
    );

    Ok(())
}
