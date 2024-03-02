use std::io;

use expect_test::expect;

use super::new_with_snapshot;

#[test]
fn test_split() -> io::Result<()> {
    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(50, 8), "x");

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 x                                            "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "scratch:1:0                                       "
            "-- NORMAL --                                      "
        "#]],
    );

    editor.split(zi::Direction::Right);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 x                      1 x                   "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "scratch:1:0                                       "
            "-- NORMAL --                                      "
        "#]],
    );

    editor.split(zi::Direction::Right);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 x              1 x             1 x           "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                                  "
            "scratch:1:0                                       "
            "-- NORMAL --                                      "
        "#]],
    );

    editor.split(zi::Direction::Down);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 x              1 x             1 x           "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                    1 x           "
            "                                                  "
            "scratch:1:0                                       "
            "-- NORMAL --                                      "
        "#]],
    );

    editor.split(zi::Direction::Left);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 x              1 x             1 x           "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                    1 x      1 x  "
            "                                                  "
            "scratch:1:0                                       "
            "-- NORMAL --                                      "
        "#]],
    );

    editor.split(zi::Direction::Up);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 x              1 x             1 x           "
            "                                                  "
            "                                                  "
            "                                                  "
            "                                    1 x      1 x  "
            "                                                  "
            "scratch:1:0                         1 x           "
            "-- NORMAL --                                      "
        "#]],
    );

    Ok(())
}
