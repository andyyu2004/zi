use super::*;

#[test]
fn text_impls() {
    // Test against the rope implementation as that one is probably correct

    // proptest this
    for s in [""] {
        let rope = Rope::from(s);
        let reference = &rope as &dyn Text;

        for imp in [s] {
            assert_eq!(reference.len_chars(), imp.len_chars());
            for i in 0..imp.len_chars() {
                assert_eq!(reference.get_char(i), imp.get_char(i));
                assert_eq!(reference.char_to_line(i), imp.line_to_char(i));
            }

            assert_eq!(reference.len_lines(), imp.len_lines());
            for i in 0..imp.len_lines() {
                assert_eq!(reference.get_line(i), imp.get_line(i));
                assert_eq!(reference.line_to_char(i), imp.line_to_char(i));
            }
        }
    }
}
