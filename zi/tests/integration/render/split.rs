use std::io;

use expect_test::expect;
use zi::Direction::*;

use super::new_with_snapshot;

#[test]
fn close_view() {
    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(51, 8), "1\n2\n3\n");
    editor.split_active_view(Right);
    editor.close_active_view();
    assert!(!editor.should_quit());

    snapshot(
        &mut editor,
        expect![[r#"
        "   1 1                                             "
        "   2 2                                             "
        "   3 3                                             "
        "   4 |                                             "
        "                                                   "
        "                                                   "
        "scratch:4:0                                        "
        "-- INSERT --                                       "
    "#]],
    );

    editor.split_active_view(Right);
    editor.split_active_view(Down);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 1                       1 1                   "
            "   2 2                       2 2                   "
            "   3 3                       3 3                   "
            "   4                         1 1                   "
            "                             2 2                   "
            "                             3 3                   "
            "scratch:4:0                    |                   "
            "-- INSERT --                                       "
        "#]],
    );

    editor.close_active_view();

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

    editor.close_active_view();
    assert!(!editor.should_quit());

    editor.close_active_view();
    assert!(editor.should_quit());
}

#[test]
fn splits_have_independent_scroll() -> io::Result<()> {
    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(51, 8), "1\n2\n3\n");

    editor.split_active_view(Right);

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

    editor.scroll_active_view(Down, 1);

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

    editor.split_active_view(Down);

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

    editor.split_active_view(Right);

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

    editor.split_active_view(Right);
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

    editor.split_active_view(Down);
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

    editor.split_active_view(Left);
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

    editor.split_active_view(Up);
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

#[test]
fn more_splits() {
    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(20, 8), "abc");

    let a = editor.active_view().id();
    editor.split_active_view(Right);
    editor.split_active_view(Down);
    editor.focus_view(a);
    editor.split_active_view(Down);
    editor.split_active_view(Down);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc     1 abc  "
            "                    "
            "   1 abc            "
            "             1 abc  "
            "   1 abc|           "
            "                    "
            "scratch:1:3         "
            "-- INSERT --        "
        "#]],
    );
}

#[test]
fn test_directional_focus() {
    // regression test for cb801c66734ff16be921087a982b53fa626a976a

    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(24, 6), "abc");
    let a = editor.active_view().id();
    let b = editor.split_active_view(Right);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc       1 abc|   "
            "                        "
            "                        "
            "                        "
            "scratch:1:3             "
            "-- INSERT --            "
        "#]],
    );

    assert_eq!(editor.move_focus(Left), a);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc|      1 abc    "
            "                        "
            "                        "
            "                        "
            "scratch:1:3             "
            "-- INSERT --            "
        "#]],
    );

    let _c = editor.split_active_view(Down);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc       1 abc    "
            "                        "
            "   1 abc|               "
            "                        "
            "scratch:1:3             "
            "-- INSERT --            "
        "#]],
    );

    assert_eq!(editor.move_focus(Right), b);
    snapshot(
        &mut editor,
        expect![[r#"
            "   1 abc       1 abc|   "
            "                        "
            "   1 abc                "
            "                        "
            "scratch:1:3             "
            "-- INSERT --            "
        "#]],
    );
}

#[test]
fn test_directional_focus_propagation() {
    // regression test for cb801c66734ff16be921087a982b53fa626a976a

    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(32, 6), "ab");
    editor.split_active_view(Right);
    editor.split_active_view(Down);
    editor.split_active_view(Right);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 ab            1 ab         "
            "                                "
            "                   1 ab    1 ab|"
            "                                "
            "scratch:1:2                     "
            "-- INSERT --                    "
        "#]],
    );

    editor.move_focus(Left);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 ab            1 ab         "
            "                                "
            "                   1 ab|   1 ab "
            "                                "
            "scratch:1:2                     "
            "-- INSERT --                    "
        "#]],
    );

    // If the focus can't move any further in the requested direction, it should propagate up.
    editor.move_focus(Left);

    snapshot(
        &mut editor,
        expect![[r#"
            "   1 ab|           1 ab         "
            "                                "
            "                   1 ab    1 ab "
            "                                "
            "scratch:1:2                     "
            "-- INSERT --                    "
        "#]],
    );
}
