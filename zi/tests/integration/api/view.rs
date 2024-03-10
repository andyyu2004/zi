use zi::Constraint::*;
use zi::Direction::*;

use super::new;

#[test]
fn test_split() {
    let mut editor = new("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");

    let left = editor.active_view().id();
    let right = editor.split_active_view(Right, Fill(1));
    assert_ne!(left, right, "splitting should create a new view");
    assert_eq!(editor.active_view().id(), right, "new view should be active");

    // should preserve cursor and offsets
    assert_eq!(editor.view(left).cursor(), editor.view(right).cursor());
    assert_eq!(editor.view(left).offset(), editor.view(right).offset());

    editor.scroll_active_view(zi::Direction::Down, 1);
    assert_ne!(editor.view(left).offset(), editor.view(right).offset());
}

#[test]
fn test_directional_focus() {
    let mut editor = new("");
    let a = editor.active_view().id();
    assert_eq!(editor.move_focus(Up), a);
    assert_eq!(editor.move_focus(Down), a);
    assert_eq!(editor.move_focus(Right), a);
    assert_eq!(editor.move_focus(Left), a);

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

    let b = editor.split_active_view(Right, Fill(1));
    let c = editor.split_active_view(Down, Fill(1));
    editor.focus_view(a);
    assert_eq!(editor.active_view().id(), a);
    let d = editor.split_active_view(Down, Fill(1));
    let e = editor.split_active_view(Down, Fill(1));

    assert_eq!(editor.active_view().id(), e);

    let mut check = #[track_caller]
    |direction, expected| {
        assert_eq!(editor.move_focus(direction), expected);
        assert_eq!(editor.active_view().id(), expected);
    };

    check(Up, d);
    check(Right, b);
    check(Down, c);
    check(Down, c);
    check(Left, a);
}
