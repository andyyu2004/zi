use std::io;

use expect_test::expect;
use zi::Constraint::*;
use zi::Direction::*;

use crate::render::new;

#[tokio::test]
async fn view_only() {
    let cx = new(zi::Size::new(51, 8), "1\n2\n3\n").await;

    cx.with(|editor| {
        let v = editor.split(zi::Active, Right, Fill(1));
        editor.split(zi::Active, Down, Fill(1));
        editor.view_only(v);
    })
    .await;

    cx.snapshot(expect![[r#"
        "   1 1                                             "
        "   2 2                                             "
        "   3 3                                             "
        "   4 |                                             "
        "                                                   "
        "                                                   "
        "scratch:4:0                                        "
        "-- INSERT --                                       "
    "#]])
        .await;
}

#[tokio::test]
async fn close_view() {
    let cx = new(zi::Size::new(51, 8), "1\n2\n3\n").await;

    cx.with(|editor| {
        let v = editor.split(zi::Active, Right, Fill(1));
        editor.close_view(v);
    })
    .await;

    cx.snapshot(expect![[r#"
        "   1 1                                             "
        "   2 2                                             "
        "   3 3                                             "
        "   4 |                                             "
        "                                                   "
        "                                                   "
        "scratch:4:0                                        "
        "-- INSERT --                                       "
    "#]])
        .await;

    cx.with(|editor| {
        editor.split(zi::Active, Right, Fill(1));
        editor.split(zi::Active, Down, Fill(1));
    })
    .await;

    cx.snapshot(expect![[r#"
        "   1 1                       1 1                   "
        "   2 2                       2 2                   "
        "   3 3                       3 3                   "
        "   4                         1 1                   "
        "                             2 2                   "
        "                             3 3                   "
        "scratch:4:0                    |                   "
        "-- INSERT --                                       "
    "#]])
        .await;

    cx.with(|editor| editor.close_view(zi::Active)).await;
    cx.snapshot(expect![[r#"
        "   1 1                       1 1                   "
        "   2 2                       2 2                   "
        "   3 3                       3 3                   "
        "   4                         4 |                   "
        "                                                   "
        "                                                   "
        "scratch:4:0                                        "
        "-- INSERT --                                       "
    "#]])
        .await;
}

#[tokio::test]
async fn splits_have_independent_scroll() -> io::Result<()> {
    let cx = new(zi::Size::new(51, 8), "1\n2\n3\n").await;

    cx.with(|editor| editor.split(zi::Active, Right, Fill(1))).await;
    cx.snapshot(expect![[r#"
        "   1 1                       1 1                   "
        "   2 2                       2 2                   "
        "   3 3                       3 3                   "
        "   4                         4 |                   "
        "                                                   "
        "                                                   "
        "scratch:4:0                                        "
        "-- INSERT --                                       "
    "#]])
        .await;

    cx.with(|editor| editor.scroll(zi::Active, Down, 1)).await;
    cx.snapshot(expect![[r#"
        "   1 1                       2 2                   "
        "   2 2                       3 3                   "
        "   3 3                       4 |                   "
        "   4                                               "
        "                                                   "
        "                                                   "
        "scratch:4:0                                        "
        "-- INSERT --                                       "
    "#]])
        .await;

    Ok(())
}

#[tokio::test]
async fn split() -> io::Result<()> {
    let cx = new(zi::Size::new(50, 8), "abc").await;

    cx.with(|editor| editor.split(zi::Active, Down, Fill(1))).await;
    cx.snapshot(expect![[r#"
        "   1 abc                                          "
        "                                                  "
        "                                                  "
        "   1 abc|                                         "
        "                                                  "
        "                                                  "
        "scratch:1:3                                       "
        "-- INSERT --                                      "
    "#]])
        .await;

    let cx = new(zi::Size::new(50, 8), "abc").await;
    cx.with(|editor| editor.split(zi::Active, Right, Fill(1))).await;
    cx.snapshot(expect![[r#"
        "   1 abc                    1 abc|                "
        "                                                  "
        "                                                  "
        "                                                  "
        "                                                  "
        "                                                  "
        "scratch:1:3                                       "
        "-- INSERT --                                      "
    "#]])
        .await;

    cx.with(|editor| editor.split(zi::Active, Right, Fill(1))).await;
    cx.snapshot(expect![[r#"
        "   1 abc            1 abc           1 abc|        "
        "                                                  "
        "                                                  "
        "                                                  "
        "                                                  "
        "                                                  "
        "scratch:1:3                                       "
        "-- INSERT --                                      "
    "#]])
        .await;

    cx.with(|editor| editor.split(zi::Active, Down, Fill(1))).await;
    cx.snapshot(expect![[r#"
        "   1 abc            1 abc           1 abc         "
        "                                                  "
        "                                                  "
        "                                    1 abc|        "
        "                                                  "
        "                                                  "
        "scratch:1:3                                       "
        "-- INSERT --                                      "
    "#]])
        .await;

    cx.with(|editor| editor.split(zi::Active, Left, Fill(1))).await;
    cx.snapshot(expect![[r#"
        "   1 abc            1 abc           1 abc         "
        "                                                  "
        "                                                  "
        "                                    1 abc|   1 abc"
        "                                                  "
        "                                                  "
        "scratch:1:3                                       "
        "-- INSERT --                                      "
    "#]])
        .await;

    cx.with(|editor| editor.split(zi::Active, Up, Fill(1))).await;
    cx.snapshot(expect![[r#"
        "   1 abc            1 abc           1 abc         "
        "                                                  "
        "                                                  "
        "                                    1 abc|   1 abc"
        "                                                  "
        "                                    1 abc         "
        "scratch:1:3                                       "
        "-- INSERT --                                      "
    "#]])
        .await;

    Ok(())
}

#[tokio::test]
async fn more_splits() {
    let cx = new(zi::Size::new(20, 8), "abc").await;
    cx.with(|editor| {
        let a = editor.view(zi::Active).id();
        editor.split(zi::Active, Right, Fill(1));
        editor.split(zi::Active, Down, Fill(1));
        editor.focus(a);
        editor.split(zi::Active, Down, Fill(1));
        editor.split(zi::Active, Down, Fill(1));
    })
    .await;

    cx.snapshot(expect![[r#"
        "   1 abc     1 abc  "
        "                    "
        "   1 abc            "
        "             1 abc  "
        "   1 abc|           "
        "                    "
        "scratch:1:3         "
        "-- INSERT --        "
    "#]])
        .await;
}

#[tokio::test]
async fn test_directional_focus() {
    // regression test for cb801c66734ff16be921087a982b53fa626a976a

    let cx = new(zi::Size::new(24, 6), "abc").await;
    let (a, b) = cx
        .with(|editor| {
            let a = editor.view(zi::Active).id();
            let b = editor.split(zi::Active, Right, Fill(1));
            (a, b)
        })
        .await;

    cx.snapshot(expect![[r#"
        "   1 abc       1 abc|   "
        "                        "
        "                        "
        "                        "
        "scratch:1:3             "
        "-- INSERT --            "
    "#]])
        .await;

    cx.with(move |editor| assert_eq!(editor.focus_direction(Left), a)).await;

    cx.snapshot(expect![[r#"
        "   1 abc|      1 abc    "
        "                        "
        "                        "
        "                        "
        "scratch:1:3             "
        "-- INSERT --            "
    "#]])
        .await;

    cx.with(|editor| editor.split(zi::Active, Down, Fill(1))).await;
    cx.snapshot(expect![[r#"
        "   1 abc       1 abc    "
        "                        "
        "   1 abc|               "
        "                        "
        "scratch:1:3             "
        "-- INSERT --            "
    "#]])
        .await;

    cx.with(move |editor| assert_eq!(editor.focus_direction(Right), b)).await;

    cx.snapshot(expect![[r#"
        "   1 abc       1 abc|   "
        "                        "
        "   1 abc                "
        "                        "
        "scratch:1:3             "
        "-- INSERT --            "
    "#]])
        .await;
}

#[tokio::test]
async fn test_directional_focus_propagation() {
    // regression test for cb801c66734ff16be921087a982b53fa626a976a

    let cx = new(zi::Size::new(32, 6), "ab").await;
    cx.with(|editor| {
        editor.split(zi::Active, Right, Fill(1));
        editor.split(zi::Active, Down, Fill(1));
        editor.split(zi::Active, Right, Fill(1));
    })
    .await;

    cx.snapshot(expect![[r#"
        "   1 ab            1 ab         "
        "                                "
        "                   1 ab    1 ab|"
        "                                "
        "scratch:1:2                     "
        "-- INSERT --                    "
    "#]])
        .await;

    cx.with(|editor| editor.focus_direction(Left)).await;

    cx.snapshot(expect![[r#"
        "   1 ab            1 ab         "
        "                                "
        "                   1 ab|   1 ab "
        "                                "
        "scratch:1:2                     "
        "-- INSERT --                    "
    "#]])
        .await;

    // If the focus can't move any further in the requested direction, it should propagate up.
    cx.with(|editor| editor.focus_direction(Left)).await;

    cx.snapshot(expect![[r#"
        "   1 ab|           1 ab         "
        "                                "
        "                   1 ab    1 ab "
        "                                "
        "scratch:1:2                     "
        "-- INSERT --                    "
    "#]])
        .await;
}
