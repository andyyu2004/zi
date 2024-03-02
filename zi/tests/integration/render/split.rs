use std::io;

use expect_test::expect;

use super::new_with_snapshot;

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
