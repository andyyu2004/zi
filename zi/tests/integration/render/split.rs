use std::io;

use expect_test::expect;

use super::new_with_snapshot;

#[test]
fn test_split() -> io::Result<()> {
    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(50, 8), "abc");

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc|                                         "
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
            "   1 abc                    1 |bc                 "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "scratch:1:0                                       "
            "-- INSERT --                                      "
        "#]],
    );

    editor.split(zi::Direction::Right);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc            1 abc           1 |bc         "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "scratch:1:0                                       "
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
            "                                      |           "
            "                                    1 abc         "
            "                                                  "
            "scratch:1:0                                       "
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
            "                                      |           "
            "                                    1 abc    1 abc"
            "                                                  "
            "scratch:1:0                                       "
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
            "                                      |           "
            "                                    1 abc    1 abc"
            "                                                  "
            "scratch:1:0                         1 abc         "
            "-- INSERT --                                      "
        "#]],
    );

    Ok(())
}
