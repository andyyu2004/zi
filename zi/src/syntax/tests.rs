use super::*;

#[test]
fn test_delta_to_ts_edit() {
    for (text, delta, input) in [
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
                new_end_position: tree_sitter::Point { row: 0, column: 2 },
            },
        ),
    ] {
        let edit = delta_to_ts_edit(text, delta);
        assert_eq!(edit, input);
    }
}
