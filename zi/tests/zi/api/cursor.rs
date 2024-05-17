use zi::Direction::*;

use crate::api::new;

#[test]
fn set_cursor() {
    let mut editor = new("foo\ntest\n");
    assert_eq!(editor.cursor(zi::Active), (0, 0), "cursor should start at (0, 0)");
    assert_eq!(editor.cursor_line(), "foo");
    assert_eq!(editor.cursor_char(), Some('f'));

    editor.set_cursor(zi::Active, (0, 1));
    assert_eq!(editor.cursor(zi::Active), (0, 1));

    editor.set_cursor(zi::Active, (0, 3));
    assert_eq!(editor.cursor(zi::Active), (0, 2), "cursor should not move past end of line");

    editor.set_cursor(zi::Active, (1, 2));
    assert_eq!(editor.cursor(zi::Active), (1, 2));
    assert_eq!(editor.cursor_line(), "test");
    assert_eq!(editor.cursor_char(), Some('s'));

    editor.set_cursor(zi::Active, (1, 4));
    assert_eq!(editor.cursor(zi::Active), (1, 3), "cursor should not move past end of line");

    editor.set_cursor(zi::Active, (2, 0));
    assert_eq!(editor.cursor(zi::Active), (1, 0), "cursor should not move past end of buffer");
}

#[test]
fn cursor_viewport_coords_tabs() {
    let mut editor = new("fn main() {\n\tbar()\n}");
    editor.move_cursor(zi::Active, Down, 1);
    editor.move_cursor(zi::Active, Right, 1);
    assert_eq!(editor.cursor_viewport_coords(), (4, 1), "tab should count as 4 cells (by default)");
}

#[test]
fn cursor_viewport_coords_scroll() {
    let mut editor = new("1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n");
    assert_eq!(editor.cursor_viewport_coords(), (0, 0));
    editor.scroll(zi::Active, Down, 1);
    // The shouldn't actually have moved relative to the viewport
    assert_eq!(editor.cursor_viewport_coords(), (0, 0));
}

#[test]
fn move_cursor_empty() {
    let mut editor = new("");
    assert_eq!(editor.cursor(zi::Active), (0, 0));
    for _ in 1..10 {
        for &direction in &[Left, Right, Up, Down] {
            editor.move_cursor(zi::Active, direction, 1);
            assert_eq!(editor.cursor(zi::Active), (0, 0));
        }
    }
}

#[test]
fn move_cursor_horizontal_no_newline() {
    let mut editor = new("abc");
    assert_eq!(editor.cursor_line(), "abc");
    assert_eq!(editor.cursor(zi::Active), (0, 0));
    editor.move_cursor(zi::Active, Right, 1);
    assert_eq!(editor.cursor(zi::Active), (0, 1));
    editor.move_cursor(zi::Active, Right, 1);
    assert_eq!(editor.cursor(zi::Active), (0, 2));
    editor.move_cursor(zi::Active, Right, 1);
    assert_eq!(editor.cursor(zi::Active), (0, 2));

    editor.set_mode(zi::Mode::Insert);
    editor.move_cursor(zi::Active, Right, 1);
    assert_eq!(editor.cursor(zi::Active), (0, 3), "insert mode can move one character further");
}

#[test]
fn vertical_move_cursor_remembers_column() {
    let mut editor = new(r#"foo
test

longer line!
short

"#);

    assert_eq!(editor.cursor(zi::Active), (0, 0));
    editor.set_cursor(zi::Active, (1, 2));
    assert_eq!(editor.cursor(zi::Active), (1, 2));
    editor.move_cursor(zi::Active, Down, 1);
    assert_eq!(editor.cursor(zi::Active), (2, 0));
    editor.move_cursor(zi::Active, Down, 1);
    assert_eq!(editor.cursor(zi::Active), (3, 2), "should remember the last column");
    editor.move_cursor(zi::Active, Up, 1);
    assert_eq!(editor.cursor(zi::Active), (2, 0));
    editor.move_cursor(zi::Active, Up, 1);
    assert_eq!(editor.cursor(zi::Active), (1, 2), "should remember the last column");

    editor.set_cursor(zi::Active, (3, 11));
    assert_eq!(editor.cursor_char(), Some('!'));
    editor.move_cursor(zi::Active, Down, 1);
    assert_eq!(editor.cursor(zi::Active), (4, 4));
    editor.move_cursor(zi::Active, Down, 1);
    assert_eq!(editor.cursor(zi::Active), (5, 0));
    editor.move_cursor(zi::Active, Up, 1);
    assert_eq!(editor.cursor(zi::Active), (4, 4));
    editor.move_cursor(zi::Active, Up, 1);
    assert_eq!(editor.cursor(zi::Active), (3, 11));

    editor.move_cursor(zi::Active, Down, 1);
    assert_eq!(editor.cursor(zi::Active), (4, 4));
    // The following should reset the target column to 4, not 5 as it can't actually get there
    editor.move_cursor(zi::Active, Right, 1);
    assert_eq!(editor.cursor(zi::Active), (4, 4));
    editor.move_cursor(zi::Active, Up, 1);
    assert_eq!(editor.cursor(zi::Active), (3, 11));
}

#[test]
fn cursor_with_scroll() {
    let mut editor = new("foo\nbar\nbaz");

    editor.scroll(zi::Active, zi::Direction::Down, 2);
    assert_eq!(editor.cursor(zi::Active), (2, 0));
    assert_eq!(editor.cursor_line(), "baz");
    editor.move_cursor(zi::Active, Down, 1);

    // Cursor is already at the bottom, should not be able to move anymore.
    assert_eq!(editor.cursor(zi::Active), (2, 0));
}

#[test]
fn cursor_newline() {
    let mut editor = new("");
    editor.set_mode(zi::Mode::Insert);
    editor.insert_char(zi::Active, '\n');
    assert_eq!(editor.cursor(zi::Active), (1, 0));
}

#[test]
fn cursor_trailing_newline() {
    let mut editor = new("\n");
    editor.move_cursor(zi::Active, Down, 1);
    assert_eq!(editor.cursor(zi::Active), (0, 0));
}
