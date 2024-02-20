use zi::{Direction, Editor};

#[test]
fn set_cursor() {
    let mut editor = Editor::new("foo\ntest\n");
    assert_eq!(editor.active_cursor(), (1, 0), "cursor should start at (1, 0)");
    assert_eq!(editor.current_line(), "foo\n");
    assert_eq!(editor.current_char(), 'f');

    editor.set_active_cursor((1, 1));
    assert_eq!(editor.active_cursor(), (1, 1));

    editor.set_active_cursor((1, 3));
    assert_eq!(editor.active_cursor(), (1, 2), "cursor should not move past end of line");

    editor.set_active_cursor((2, 2));
    assert_eq!(editor.active_cursor(), (2, 2));
    assert_eq!(editor.current_line(), "test\n");
    assert_eq!(editor.current_char(), 's');

    editor.set_active_cursor((2, 4));
    assert_eq!(editor.active_cursor(), (2, 3), "cursor should not move past end of line");

    editor.set_active_cursor((3, 0));
    assert_eq!(
        editor.active_cursor(),
        (3, 0),
        "this should be ok because the line exists (but is empty)"
    );

    editor.set_active_cursor((3, 1));
    assert_eq!(editor.active_cursor(), (3, 0));

    editor.set_active_cursor((4, 0));
    assert_eq!(editor.active_cursor(), (3, 0), "cursor should not move past end of buffer");
}

#[test]
fn move_cursor_empty() {
    let mut editor = Editor::new("");
    assert_eq!(editor.active_cursor(), (1, 0));
    for _ in 1..10 {
        for &direction in &[Direction::Left, Direction::Right, Direction::Up, Direction::Down] {
            editor.move_active_cursor(direction);
            assert_eq!(editor.active_cursor(), (1, 0));
        }
    }
}

#[test]
fn move_cursor_horizontal_no_newline() {
    let mut editor = Editor::new("abc");
    assert_eq!(editor.active_cursor(), (1, 0));
    editor.move_active_cursor(Direction::Right);
    assert_eq!(editor.active_cursor(), (1, 1));
    editor.move_active_cursor(Direction::Right);
    assert_eq!(editor.active_cursor(), (1, 2));
    editor.move_active_cursor(Direction::Right);
    assert_eq!(editor.active_cursor(), (1, 2));

    editor.set_mode(zi::Mode::Insert);
    editor.move_active_cursor(Direction::Right);
    assert_eq!(editor.active_cursor(), (1, 3), "insert mode can move one character further");
}

#[test]
fn vertical_move_cursor_remembers_column() {
    let mut editor = Editor::new(
        r#"foo
test

longer line!
short

"#,
    );

    use Direction::*;
    assert_eq!(editor.active_cursor(), (1, 0));
    editor.set_active_cursor((2, 2));
    assert_eq!(editor.active_cursor(), (2, 2));
    editor.move_active_cursor(Down);
    assert_eq!(editor.active_cursor(), (3, 0));
    editor.move_active_cursor(Down);
    assert_eq!(editor.active_cursor(), (4, 2), "should remember the last column");
    editor.move_active_cursor(Up);
    assert_eq!(editor.active_cursor(), (3, 0));
    editor.move_active_cursor(Up);
    assert_eq!(editor.active_cursor(), (2, 2), "should remember the last column");

    editor.set_active_cursor((4, 11));
    assert_eq!(editor.current_char(), '!');
    editor.move_active_cursor(Down);
    assert_eq!(editor.active_cursor(), (5, 4));
    editor.move_active_cursor(Down);
    assert_eq!(editor.active_cursor(), (6, 0));
    editor.move_active_cursor(Up);
    assert_eq!(editor.active_cursor(), (5, 4));
    editor.move_active_cursor(Up);
    assert_eq!(editor.active_cursor(), (4, 11));

    editor.move_active_cursor(Down);
    assert_eq!(editor.active_cursor(), (5, 4));
    // The following should reset the target column to 4, not 5 as it can't actually get there
    editor.move_active_cursor(Right);
    assert_eq!(editor.active_cursor(), (5, 4));
    editor.move_active_cursor(Up);
    assert_eq!(editor.active_cursor(), (4, 4));
}
