use zi::Direction::*;

use crate::api::{new, new_with_size};

#[test]
fn cursor_scrolls_when_out_of_bounds() {
    let mut editor = new_with_size("1\n2\n3\n4\n5\n", zi::Size::new(10, 2));
    editor.scroll_active_view(Down, 1);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.active_view().cursor(), (1, 0));

    // scrolling the cursor off the screen should also scroll the view
    editor.move_active_cursor(Up, 1);
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.active_view().cursor(), (0, 0));

    editor.move_active_cursor(Down, 1);
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.active_view().cursor(), (1, 0));

    // scrolling down past the editor size
    editor.move_active_cursor(Down, 1);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.active_view().cursor(), (2, 0));

    editor.move_active_cursor(Down, 2);
    assert_eq!(editor.active_view().offset(), (3, 0));
    assert_eq!(editor.active_view().cursor(), (4, 0));
}

#[test]
fn scroll() {
    let mut editor = new("");
    editor.scroll_active_view(Down, 1);
    assert_eq!(editor.current_line(), "");
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.active_view().cursor(), (0, 0));

    let mut editor = new("foo\nbar");
    assert_eq!(editor.current_line(), "foo");
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.active_view().cursor(), (0, 0));

    editor.scroll_active_view(Down, 1);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.active_view().cursor(), (1, 0));
    assert_eq!(editor.current_line(), "bar");

    editor.scroll_active_view(Up, 1);
    assert_eq!(editor.current_line(), "foo");
    assert_eq!(editor.active_view().offset(), (0, 0));

    // Bounds check above
    editor.scroll_active_view(Up, 1);
    assert_eq!(editor.current_line(), "foo");
    assert_eq!(editor.active_view().offset(), (0, 0));

    // Bounds check below
    // Should not be able to scroll where all text is not visible.
    // There should always be at least one line visible.
    editor.scroll_active_view(Down, 1);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.current_line(), "bar");

    editor.scroll_active_view(Up, 50);
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.active_view().cursor(), (0, 0));
    assert_eq!(editor.current_line(), "foo");

    editor.scroll_active_view(Down, 50);
    assert_eq!(editor.active_view().offset(), (1, 0));
    assert_eq!(editor.active_view().cursor(), (1, 0));
    assert_eq!(editor.current_line(), "bar");
}

#[test]
fn scroll_bounds_check() {
    let mut editor = new("1\n2\n3\n4\n5");

    // Move cursor down one so it's desyned from the scroll
    editor.move_active_cursor(Down, 1);
    assert_eq!(editor.active_view().offset(), (0, 0));
    assert_eq!(editor.active_view().cursor(), (1, 0));

    // Scroll as far as possible
    editor.scroll_active_view(Down, 20);

    // Ensure the cursor is still following the scroll
    assert_eq!(editor.active_view().offset(), (4, 0));
    assert_eq!(editor.active_view().cursor(), (4, 0));
}
