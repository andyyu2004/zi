use expect_test::expect;
use zi::Constraint::*;
use zi::Direction::*;

use crate::new;

#[tokio::test]
async fn jump_list() {
    let cx = new("").await;
    cx.with(|editor| {
        let loc_initial = editor.current_location();
        let a = editor.create_readonly_buffer("a.txt", "aa".as_bytes());
        let b = editor.create_readonly_buffer("b.txt", "bb".as_bytes());

        // NOTE: a=2v0 b=3v0

        // jumping should store where we came from
        editor.jump_to(zi::Location::new(a, (0, 0)));
        expect![[r"
            BufferId(0v0):0:0
             <<<
        "]]
        .assert_debug_eq(&editor.view(zi::Active).jump_list());

        editor.jump_to(zi::Location::new(b, (0, 1)));
        expect![[r"
            BufferId(0v0):0:0
            BufferId(2v0):0:0
             <<<
        "]]
        .assert_debug_eq(&editor.view(zi::Active).jump_list());

        let loc_a = zi::Location::new(a, (0, 0));
        let loc_b = zi::Location::new(b, (0, 1));

        assert_eq!(editor.jump_back(zi::Active), Some(loc_a));
        assert_eq!(editor.jump_back(zi::Active), Some(loc_initial));
        assert_eq!(editor.jump_back(zi::Active), None);
        assert_eq!(editor.jump_forward(zi::Active), Some(loc_a));
        assert_eq!(editor.jump_forward(zi::Active), Some(loc_b));
        assert_eq!(editor.jump_back(zi::Active), Some(loc_a));
        assert_eq!(editor.jump_forward(zi::Active), Some(loc_b));

        assert_eq!(editor.jump_forward(zi::Active), None);
        assert_eq!(editor.jump_forward(zi::Active), None);
        assert_eq!(editor.jump_back(zi::Active), Some(loc_a));

        expect![[r"
            BufferId(0v0):0:0
            BufferId(2v0):0:0 <<<
            BufferId(3v0):0:1
        "]]
        .assert_debug_eq(&editor.view(zi::Active).jump_list());
    })
    .await;

    cx.cleanup().await;
}

#[tokio::test]
async fn view_group() {
    let cx = new("").await;
    cx.with(|editor| {
        let group = editor.create_view_group("test").unwrap();
        assert_eq!(
            editor.create_view_group("test"),
            Err(group),
            "should return the same group given the same key"
        );

        editor.set_view_group(zi::Active, group);
        assert_eq!(editor.view(zi::Active).group(), Some(group));

        let v = editor.split(zi::Active, Right, Fill(1));
        assert!(editor.view(v).group().is_none(), "split view should not copy the group");

        editor.set_view_group(v, group);

        editor.close_view(zi::Active);
    })
    .await;

    cx.cleanup().await;
}

#[tokio::test]
async fn test_split() {
    let cx = new("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n").await;
    cx.with(|editor| {
        let left = editor.view(zi::Active).id();
        let right = editor.split(zi::Active, Right, Fill(1));
        assert_ne!(left, right, "splitting should create a new view");
        assert_eq!(editor.view(zi::Active).id(), right, "new view should be active");

        // should preserve cursor and offsets
        assert_eq!(editor.view(left).cursor(), editor.view(right).cursor());
        assert_eq!(editor.view(left).offset(), editor.view(right).offset());

        editor.scroll_by(zi::Active, zi::Direction::Down, 1);
        assert_ne!(editor.view(left).offset(), editor.view(right).offset());
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn split_copies_view_settings() {
    let cx = new("").await;
    cx.with(|editor| {
        let left = editor.view(zi::Active).id();
        editor.view(left).settings().line_number_style.write(zi::LineNumberStyle::Relative);
        let right = editor.split(zi::Active, Right, Fill(1));
        assert_eq!(editor.view(right).settings().line_number_style, zi::LineNumberStyle::Relative);

        editor.view(right).settings().line_number_style.write(zi::LineNumberStyle::None);
        assert_eq!(editor.view(left).settings().line_number_style, zi::LineNumberStyle::Relative);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
#[expect(clippy::many_single_char_names, reason = "views are named as in the layout diagram")]
async fn test_directional_focus() {
    let cx = new("").await;
    cx.with(|editor| {
        let a = editor.view(zi::Active).id();
        assert_eq!(editor.focus_direction(Up), a);
        assert_eq!(editor.focus_direction(Down), a);
        assert_eq!(editor.focus_direction(Right), a);
        assert_eq!(editor.focus_direction(Left), a);

        // Setting up the following layout
        // +--------------------+
        // |          |         |
        // |   (a)    |  (b)    |
        // |----------|         |
        // +   (d)    |---------+
        // |----------|         |
        // |          |  (c)    |
        // |   (e)    |         |
        // +--------------------+

        let b = editor.split(zi::Active, Right, Fill(1));
        let c = editor.split(zi::Active, Down, Fill(1));
        editor.focus(a);
        assert_eq!(editor.view(zi::Active).id(), a);
        let d = editor.split(zi::Active, Down, Fill(1));
        let e = editor.split(zi::Active, Down, Fill(1));

        assert_eq!(editor.view(zi::Active).id(), e);

        #[track_caller]
        fn check(editor: &mut zi::Editor, direction: zi::Direction, expected: zi::ViewId) {
            assert_eq!(editor.focus_direction(direction), expected);
            assert_eq!(editor.view(zi::Active).id(), expected);
        }

        check(editor, Up, d);
        check(editor, Right, b);
        check(editor, Down, c);
        check(editor, Down, c);
        check(editor, Left, a);
    })
    .await;
    cx.cleanup().await;
}
