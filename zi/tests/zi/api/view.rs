use expect_test::expect;
use zi::Constraint::*;
use zi::Direction::*;

use super::new;

#[tokio::test]
async fn jump_list() {
    let mut editor = new("").await;
    let loc_initial = editor.current_location();
    let a = editor.create_readonly_buffer("a.txt", "aa".as_bytes());
    let b = editor.create_readonly_buffer("b.txt", "bb".as_bytes());

    // NOTE: a=3v1 b=4v1

    // jumping should store where we came from
    editor.jump_to(zi::Location::new(a, (0, 0)));
    expect![[r#"
        BufferId(3v1):0:0
         <<<
    "#]]
    .assert_debug_eq(&editor.view(zi::Active).jump_list());

    editor.jump_to(zi::Location::new(b, (0, 1)));
    expect![[r#"
        BufferId(3v1):0:0
        BufferId(4v1):0:0
         <<<
    "#]]
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

    expect![[r#"
        BufferId(3v1):0:0
        BufferId(4v1):0:0 <<<
        BufferId(5v1):0:1
    "#]]
    .assert_debug_eq(&editor.view(zi::Active).jump_list());
}

#[tokio::test]
async fn view_group() {
    let mut editor = new("").await;
    let url = zi::Url::parse("view-group://test").unwrap();
    let group = editor.create_view_group(url.clone()).unwrap();
    assert_eq!(
        editor.create_view_group(url),
        Err(group),
        "should return the same group given the same url"
    );

    editor.set_view_group(zi::Active, group);
    assert_eq!(editor.view(zi::Active).group(), Some(group));

    let v = editor.split(zi::Active, Right, Fill(1));
    assert!(editor.view(v).group().is_none(), "split view should not copy the group");

    editor.set_view_group(v, group);

    editor.close_view(zi::Active);
}

#[tokio::test]
async fn test_split() {
    let mut editor = new("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n").await;

    let left = editor.view(zi::Active).id();
    let right = editor.split(zi::Active, Right, Fill(1));
    assert_ne!(left, right, "splitting should create a new view");
    assert_eq!(editor.view(zi::Active).id(), right, "new view should be active");

    // should preserve cursor and offsets
    assert_eq!(editor.view(left).cursor(), editor.view(right).cursor());
    assert_eq!(editor.view(left).offset(), editor.view(right).offset());

    editor.scroll(zi::Active, zi::Direction::Down, 1);
    assert_ne!(editor.view(left).offset(), editor.view(right).offset());
}

#[tokio::test]
async fn test_directional_focus() {
    let mut editor = new("").await;
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

    let mut check = #[track_caller]
    |direction, expected| {
        assert_eq!(editor.focus_direction(direction), expected);
        assert_eq!(editor.view(zi::Active).id(), expected);
    };

    check(Up, d);
    check(Right, b);
    check(Down, c);
    check(Down, c);
    check(Left, a);
}
