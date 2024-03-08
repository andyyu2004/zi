use super::*;

#[test]
fn str_text_impl() {
    assert_eq!("".len_lines(), 1);
    assert_eq!("".get_line(0), Some("".into()));
    assert_eq!("x".len_lines(), 1);
    assert_eq!("\n".len_lines(), 2);
    assert_eq!("\n".get_line(1), Some("".into()));
    assert_eq!("\n".get_char(0), Some('\n'));
    assert_eq!("\nx".get_char(1), Some('x'));
    assert_eq!("\nx".char_to_line(0), 0);
    assert_eq!("\nx".char_to_line(1), 1);
    assert_eq!("\n".line_to_char(0), 0);
    assert_eq!("\n".line_to_char(1), 1);
    assert_eq!("a\n".line_to_char(0), 0);
    assert_eq!("a\n".line_to_char(1), 2);
}

proptest::proptest! {
    // Ignore some annoying control characters like vertical tabs, no idea if anyone actually uses that in practice.
    // Also skipping \r as usually it's followed by \n.
    #[test]
    fn text_impls(s in "[^\r\u{b}\u{c}]*") {
        // Test against the rope implementation as that one is probably correct

        let rope = Rope::from(s.as_ref());
        let reference = &rope as &dyn Text;

        for imp in [&s] {
            assert_eq!(reference.len_chars(), imp.len_chars());
            for c in 0..imp.len_chars() {
                assert_eq!(reference.get_char(c), imp.get_char(c), "{s:?}: char {c}" );
                assert_eq!(reference.char_to_line(c), imp.char_to_line(c), "{s:?}: char {c}");
            }

            assert_eq!(reference.len_lines(), imp.len_lines());
            for l in 0..imp.len_lines() {
                assert_eq!(reference.get_line(l), imp.get_line(l), "{s:?}: on line {l}");
                assert_eq!(reference.line_to_char(l), imp.line_to_char(l), "{s:?}`: on line {l}");
            }
        }
    }
}
