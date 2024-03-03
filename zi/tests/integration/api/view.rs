use super::new;

#[test]
fn test_split() {
    let mut editor = new("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n");

    let left = editor.active_view().id();
    let right = editor.split_active_view(zi::Direction::Right);
    assert_ne!(left, right, "splitting should create a new view");
    assert_eq!(editor.active_view().id(), right, "new view should be active");

    // should preserve cursor and offsets
    assert_eq!(editor.view(left).cursor(), editor.view(right).cursor());
    assert_eq!(editor.view(left).offset(), editor.view(right).offset());

    editor.scroll_active_view(zi::Direction::Down, 1);
    assert_ne!(editor.view(left).offset(), editor.view(right).offset());
}
