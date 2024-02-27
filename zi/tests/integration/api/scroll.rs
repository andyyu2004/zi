use zi::Direction::*;

use crate::api::new;

#[test]
fn test_cursor_scrolls_when_out_of_bounds() {
    let mut editor = new("foo\nbar\n");
    editor.scroll_active_view(Down, 1);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.active_view().cursor(), (1, 0));

    // scrolling the cursor off the screen should also scroll the view
    editor.move_active_cursor(Up, 1);
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.active_view().cursor(), (0, 0));
}

#[test]
fn test_scroll() {
    let mut editor = new("");
    editor.scroll_active_view(Down, 1);
    assert_eq!(editor.current_line(), "");
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.active_view().cursor(), (0, 0));

    let mut editor = new("foo\nbar\n");
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.active_view().cursor(), (0, 0));

    editor.scroll_active_view(Down, 1);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.active_view().cursor(), (1, 0));
    assert_eq!(editor.current_line(), "bar\n");

    editor.scroll_active_view(Up, 1);
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.active_view().offset(), (0, 0));

    // Bounds check above
    editor.scroll_active_view(Up, 1);
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.active_view().offset(), (0, 0));

    // Not allowed to go on the empty line
    editor.scroll_active_view(Down, 2);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.current_line(), "bar\n");

    // Bounds check below
    // Should not be able to scroll where all text is not visible.
    // There should always be at least one line visible.
    editor.scroll_active_view(Down, 1);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.current_line(), "bar\n");

    editor.scroll_active_view(Up, 50);
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.current_line(), "foo\n");

    editor.scroll_active_view(Down, 50);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.current_line(), "bar\n");
}
