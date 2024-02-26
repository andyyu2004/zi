use crate::api::new;

#[test]
fn test_scroll() {
    let mut editor = new("");
    editor.scroll(zi::Direction::Down, 1);
    assert_eq!(editor.current_line(), "");
    assert_eq!(editor.active_view().offset(), (0, 0));

    let mut editor = new("foo\nbar\n");
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.active_view().offset(), (0, 0));

    editor.scroll(zi::Direction::Down, 1);
    assert_eq!(editor.current_line(), "bar\n");
    assert_eq!(editor.active_view().offset(), (1, 0));

    editor.scroll(zi::Direction::Up, 1);
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.active_view().offset(), (0, 0));

    // Bounds check above
    editor.scroll(zi::Direction::Up, 1);
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.active_view().offset(), (0, 0));

    // Not allowed to go on the empty line
    editor.scroll(zi::Direction::Down, 2);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.current_line(), "bar\n");

    // Bounds check below
    // Should not be able to scroll where all text is not visible.
    // There should always be at least one line visible.
    editor.scroll(zi::Direction::Down, 1);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.current_line(), "bar\n");

    editor.scroll(zi::Direction::Up, 50);
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.current_line(), "foo\n");

    editor.scroll(zi::Direction::Down, 50);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.current_line(), "bar\n");
}
