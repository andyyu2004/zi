use zi::Direction::*;

use crate::api::new;

#[test]
fn set_cursor() {
    let mut editor = new("foo\ntest\n\n");
    assert_eq!(editor.active_cursor(), (0, 0), "cursor should start at (1, 0)");
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.current_char(), 'f');

    editor.set_active_cursor((0, 1));
    assert_eq!(editor.active_cursor(), (0, 1));

    editor.set_active_cursor((0, 3));
    assert_eq!(editor.active_cursor(), (0, 2), "cursor should not move past end of line");

    editor.set_active_cursor((1, 2));
    assert_eq!(editor.active_cursor(), (1, 2));
    assert_eq!(editor.current_line(), "test\n");
    assert_eq!(editor.current_char(), 's');

    editor.set_active_cursor((1, 4));
    assert_eq!(editor.active_cursor(), (1, 3), "cursor should not move past end of line");

    editor.set_active_cursor((2, 0));
    assert_eq!(
        editor.active_cursor(),
        (2, 0),
        "this should be ok because the line exists (but is empty)"
    );

    editor.set_active_cursor((2, 1));
    assert_eq!(editor.active_cursor(), (2, 0));

    editor.set_active_cursor((3, 0));
    assert_eq!(editor.active_cursor(), (2, 0), "cursor should not move past end of buffer");
}

#[test]
fn cursor_is_not_allowed_on_final_empty_line() {
    let mut editor = new("foo\n");
    editor.set_active_cursor((1, 0));
    assert_eq!(
        editor.active_cursor(),
        (0, 0),
        "cursor should not be allowed on the final empty line"
    );
}

#[test]
fn cursor_viewport_coords_tabs() {
    let mut editor = new("fn main() {\n\tbar()\n}");
    editor.move_active_cursor(Down, 1);
    editor.move_active_cursor(Right, 1);
    assert_eq!(editor.cursor_viewport_coords(), (4, 1), "tab should count as 4 cells (by default)");
}

#[test]
fn cursor_viewport_coords_scroll() {
    let mut editor = new("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n");
    assert_eq!(editor.cursor_viewport_coords(), (0, 0));
    editor.scroll_active_view(Down, 1);
    // The shouldn't actually have moved relative to the viewport
    assert_eq!(editor.cursor_viewport_coords(), (0, 0));
}

#[test]
fn move_cursor_empty() {
    let mut editor = new("");
    assert_eq!(editor.active_cursor(), (0, 0));
    for _ in 1..10 {
        for &direction in &[Left, Right, Up, Down] {
            editor.move_active_cursor(direction, 1);
            assert_eq!(editor.active_cursor(), (0, 0));
        }
    }
}

#[test]
fn move_cursor_horizontal_no_newline() {
    let mut editor = new("abc");
    assert_eq!(editor.active_cursor(), (0, 0));
    editor.move_active_cursor(Right, 1);
    assert_eq!(editor.active_cursor(), (0, 1));
    editor.move_active_cursor(Right, 1);
    assert_eq!(editor.active_cursor(), (0, 2));
    editor.move_active_cursor(Right, 1);
    assert_eq!(editor.active_cursor(), (0, 2));

    editor.set_mode(zi::Mode::Insert);
    editor.move_active_cursor(Right, 1);
    assert_eq!(editor.active_cursor(), (0, 3), "insert mode can move one character further");
}

#[test]
fn vertical_move_cursor_remembers_column() {
    let mut editor = new(r#"foo
test

longer line!
short

"#);

    assert_eq!(editor.active_cursor(), (0, 0));
    editor.set_active_cursor((1, 2));
    assert_eq!(editor.active_cursor(), (1, 2));
    editor.move_active_cursor(Down, 1);
    assert_eq!(editor.active_cursor(), (2, 0));
    editor.move_active_cursor(Down, 1);
    assert_eq!(editor.active_cursor(), (3, 2), "should remember the last column");
    editor.move_active_cursor(Up, 1);
    assert_eq!(editor.active_cursor(), (2, 0));
    editor.move_active_cursor(Up, 1);
    assert_eq!(editor.active_cursor(), (1, 2), "should remember the last column");

    editor.set_active_cursor((3, 11));
    assert_eq!(editor.current_char(), '!');
    editor.move_active_cursor(Down, 1);
    assert_eq!(editor.active_cursor(), (4, 4));
    editor.move_active_cursor(Down, 1);
    assert_eq!(editor.active_cursor(), (5, 0));
    editor.move_active_cursor(Up, 1);
    assert_eq!(editor.active_cursor(), (4, 4));
    editor.move_active_cursor(Up, 1);
    assert_eq!(editor.active_cursor(), (3, 11));

    editor.move_active_cursor(Down, 1);
    assert_eq!(editor.active_cursor(), (4, 4));
    // The following should reset the target column to 4, not 5 as it can't actually get there
    editor.move_active_cursor(Right, 1);
    assert_eq!(editor.active_cursor(), (4, 4));
    editor.move_active_cursor(Up, 1);
    assert_eq!(editor.active_cursor(), (3, 4));
}

#[test]
fn cursor_with_scroll() {
    let mut editor = new("foo\nbar\nbaz\n");

    editor.scroll_active_view(zi::Direction::Down, 2);
    assert_eq!(editor.active_cursor(), (2, 0));
    assert_eq!(editor.current_line(), "baz\n");
    editor.move_active_cursor(Down, 1);

    // Cursor is already at the bottom, should not be able to move anymore.
    assert_eq!(editor.active_cursor(), (2, 0));
}
