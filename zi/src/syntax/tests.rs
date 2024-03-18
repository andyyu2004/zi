use ropey::Rope;

use super::*;

#[test]
fn test_delta_to_ts_edit() {
    for (text, delta, input, expected) in [
        (
            "",
            &Delta::new(0..0, "a"),
            tree_sitter::InputEdit {
                start_byte: 0,
                old_end_byte: 0,
                new_end_byte: 1,
                start_position: tree_sitter::Point { row: 0, column: 0 },
                old_end_position: tree_sitter::Point { row: 0, column: 0 },
                new_end_position: tree_sitter::Point { row: 0, column: 1 },
            },
            "a",
        ),
        (
            "x",
            &Delta::new(0..1, "a"),
            tree_sitter::InputEdit {
                start_byte: 0,
                old_end_byte: 1,
                new_end_byte: 1,
                start_position: tree_sitter::Point { row: 0, column: 0 },
                old_end_position: tree_sitter::Point { row: 0, column: 1 },
                new_end_position: tree_sitter::Point { row: 0, column: 1 },
            },
            "a",
        ),
        (
            "a",
            &Delta::new(1..1, "b"),
            tree_sitter::InputEdit {
                start_byte: 1,
                old_end_byte: 1,
                new_end_byte: 2,
                start_position: tree_sitter::Point { row: 0, column: 1 },
                old_end_position: tree_sitter::Point { row: 0, column: 1 },
                new_end_position: tree_sitter::Point { row: 0, column: 2 },
            },
            "ab",
        ),
        (
            "b",
            &Delta::new(0..0, "a"),
            tree_sitter::InputEdit {
                start_byte: 0,
                old_end_byte: 0,
                new_end_byte: 1,
                start_position: tree_sitter::Point { row: 0, column: 0 },
                old_end_position: tree_sitter::Point { row: 0, column: 0 },
                new_end_position: tree_sitter::Point { row: 0, column: 1 },
            },
            "ab",
        ),
        (
            "a",
            &Delta::new(0..1, "b\nc"),
            tree_sitter::InputEdit {
                start_byte: 0,
                old_end_byte: 1,
                new_end_byte: 3,
                start_position: tree_sitter::Point { row: 0, column: 0 },
                old_end_position: tree_sitter::Point { row: 0, column: 1 },
                new_end_position: tree_sitter::Point { row: 1, column: 1 },
            },
            "b\nc",
        ),
        (
            "a",
            &Delta::new(0..1, "©"),
            tree_sitter::InputEdit {
                start_byte: 0,
                old_end_byte: 1,
                new_end_byte: 2,
                start_position: tree_sitter::Point { row: 0, column: 0 },
                old_end_position: tree_sitter::Point { row: 0, column: 1 },
                new_end_position: tree_sitter::Point { row: 0, column: 1 },
            },
            "©",
        ),
    ] {
        let mut rope = Rope::from(text);
        let actual = delta_to_ts_edit(&mut rope, delta).unwrap();
        assert_eq!(actual, input);
        assert_eq!(rope.to_string(), expected);
    }
}
