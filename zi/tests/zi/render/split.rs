use std::io;

use expect_test::expect;
use zi::Constraint::*;
use zi::Direction::*;

use super::run;

#[tokio::test]
async fn view_only() {
    run(zi::Size::new(51, 8), "1\n2\n3\n", |editor, mut snapshot| {
        let v = editor.split(zi::Active, Right, Fill(1));
        editor.split(zi::Active, Down, Fill(1));
        editor.view_only(v);

        snapshot(
            editor,
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
    })
    .await;
}

#[tokio::test]
async fn close_view() {
    run(zi::Size::new(51, 8), "1\n2\n3\n", |editor, mut snapshot| {
        editor.split(zi::Active, Right, Fill(1));
        editor.close_view(zi::Active);
        assert!(!editor.should_quit());

        snapshot(
            editor,
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

        editor.split(zi::Active, Right, Fill(1));
        editor.split(zi::Active, Down, Fill(1));

        snapshot(
            editor,
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

        editor.close_view(zi::Active);

        snapshot(
            editor,
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

        editor.close_view(zi::Active);
        assert!(!editor.should_quit());

        editor.close_view(zi::Active);
        assert!(editor.should_quit());
    })
    .await;
}

#[tokio::test]
async fn splits_have_independent_scroll() -> io::Result<()> {
    run(zi::Size::new(51, 8), "1\n2\n3\n", |editor, mut snapshot| {
        editor.split(zi::Active, Right, Fill(1));

        snapshot(
            editor,
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

        editor.scroll(zi::Active, Down, 1);

        snapshot(
            editor,
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
    })
    .await;

    Ok(())
}

#[tokio::test]
async fn split() -> io::Result<()> {
    run(zi::Size::new(50, 8), "abc", |editor, mut snapshot| {
        editor.split(zi::Active, Down, Fill(1));

        snapshot(
            editor,
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
    })
    .await;

    run(zi::Size::new(50, 8), "abc", |editor, mut snapshot| {
        editor.split(zi::Active, Right, Fill(1));

        snapshot(
            editor,
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

        editor.split(zi::Active, Right, Fill(1));
        snapshot(
            editor,
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

        editor.split(zi::Active, Down, Fill(1));
        snapshot(
            editor,
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

        editor.split(zi::Active, Left, Fill(1));
        snapshot(
            editor,
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

        editor.split(zi::Active, Up, Fill(1));
        snapshot(
            editor,
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
    })
    .await;

    Ok(())
}

#[tokio::test]
async fn more_splits() {
    run(zi::Size::new(20, 8), "abc", |editor, mut snapshot| {
        let a = editor.view(zi::Active).id();
        editor.split(zi::Active, Right, Fill(1));
        editor.split(zi::Active, Down, Fill(1));
        editor.focus(a);
        editor.split(zi::Active, Down, Fill(1));
        editor.split(zi::Active, Down, Fill(1));

        snapshot(
            editor,
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
    })
    .await;
}

#[tokio::test]
async fn test_directional_focus() {
    // regression test for cb801c66734ff16be921087a982b53fa626a976a

    run(zi::Size::new(24, 6), "abc", |editor, mut snapshot| {
        let a = editor.view(zi::Active).id();
        let b = editor.split(zi::Active, Right, Fill(1));

        snapshot(
            editor,
            expect![[r#"
            "   1 abc       1 abc|   "
            "                        "
            "                        "
            "                        "
            "scratch:1:3             "
            "-- INSERT --            "
        "#]],
        );

        assert_eq!(editor.focus_direction(Left), a);

        snapshot(
            editor,
            expect![[r#"
            "   1 abc|      1 abc    "
            "                        "
            "                        "
            "                        "
            "scratch:1:3             "
            "-- INSERT --            "
        "#]],
        );

        let _c = editor.split(zi::Active, Down, Fill(1));
        snapshot(
            editor,
            expect![[r#"
            "   1 abc       1 abc    "
            "                        "
            "   1 abc|               "
            "                        "
            "scratch:1:3             "
            "-- INSERT --            "
        "#]],
        );

        assert_eq!(editor.focus_direction(Right), b);
        snapshot(
            editor,
            expect![[r#"
            "   1 abc       1 abc|   "
            "                        "
            "   1 abc                "
            "                        "
            "scratch:1:3             "
            "-- INSERT --            "
        "#]],
        );
    })
    .await;
}

#[tokio::test]
async fn test_directional_focus_propagation() {
    // regression test for cb801c66734ff16be921087a982b53fa626a976a

    run(zi::Size::new(32, 6), "ab", |editor, mut snapshot| {
        editor.split(zi::Active, Right, Fill(1));
        editor.split(zi::Active, Down, Fill(1));
        editor.split(zi::Active, Right, Fill(1));

        snapshot(
            editor,
            expect![[r#"
            "   1 ab            1 ab         "
            "                                "
            "                   1 ab    1 ab|"
            "                                "
            "scratch:1:2                     "
            "-- INSERT --                    "
        "#]],
        );

        editor.focus_direction(Left);

        snapshot(
            editor,
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
        editor.focus_direction(Left);

        snapshot(
            editor,
            expect![[r#"
            "   1 ab|           1 ab         "
            "                                "
            "                   1 ab    1 ab "
            "                                "
            "scratch:1:2                     "
            "-- INSERT --                    "
        "#]],
        );
    })
    .await;
}
