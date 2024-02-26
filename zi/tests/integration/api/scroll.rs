use crate::api::new;

#[test]
fn test_scroll() {
    let mut editor = new("");
    editor.scroll(zi::Direction::Down, 1);
    assert_eq!(editor.current_line(), "");
    assert_eq!(editor.active_view().offset(), (0, 0));

    let mut editor = new("foo\ntest\n");
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.active_view().offset(), (0, 0));

    editor.scroll(zi::Direction::Down, 1);
    assert_eq!(editor.current_line(), "test\n");
    assert_eq!(editor.active_view().offset(), (1, 0));

    editor.scroll(zi::Direction::Up, 1);
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.active_view().offset(), (0, 0));

    // bounds check above
    editor.scroll(zi::Direction::Up, 1);
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.active_view().offset(), (0, 0));

    // Allowed to go on the empty line
    editor.scroll(zi::Direction::Down, 2);
    assert_eq!(editor.active_view().offset(), (2, 0));
    assert_eq!(editor.current_line(), "");

    // bounds check below
    editor.scroll(zi::Direction::Down, 1);
    assert_eq!(editor.active_view().offset(), (2, 0));
    assert_eq!(editor.current_line(), "");

    editor.scroll(zi::Direction::Up, 50);
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.current_line(), "foo\n");

    editor.scroll(zi::Direction::Down, 50);
    assert_eq!(editor.active_view().offset(), (2, 0));
    assert_eq!(editor.current_line(), "");
}
