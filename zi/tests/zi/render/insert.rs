use expect_test::expect;

use super::new_with_snapshot;

#[test]
fn insert_newline_in_blank_buffer() {
    let (mut editor, mut snapshot) = new_with_snapshot(zi::Size::new(10, 5), "");

    snapshot(
        &mut editor,
        expect![[r#"
            "  1 |     "
            "          "
            "          "
            "scratch:1:"
            "-- INSERT "
        "#]],
    );

    editor.insert_char_at_cursor('\n');
    snapshot(
        &mut editor,
        expect![[r#"
            "  1       "
            "  2 |     "
            "          "
            "scratch:2:"
            "-- INSERT "
        "#]],
    );
}
