use expect_test::{expect, Expect};

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
    // Ignore some annoying control characters like vertical tabs, nextline etc. No idea if anyone actually uses that in practice.
    // Also skipping \r as usually it's followed by \n.
    #[test]
    fn text_impls(s in "[^\r\u{b}\u{c}\u{85}]*") {
        // Test against the rope implementation as that one is probably correct

        let rope = Rope::from(s.as_ref());
        let reference = &rope as &dyn Text;

        for imp in [&s.as_str() as &dyn Text, &ReadonlyText::new(s.as_bytes())] {
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

#[test]
fn text_annotations() {
    #[track_caller]
    fn check<T: Copy + fmt::Display>(
        text: impl Text,
        highlights: impl IntoIterator<Item = (&'static str, T)>,
        expect: Expect,
    ) {
        let highlights = highlights.into_iter().map(|(range, annotation)| {
            let range = range.parse().unwrap();
            (range, annotation)
        });

        let chunks = text.annotate(highlights).collect::<Vec<_>>();
        let mut s = String::new();
        for (_, text, ann) in chunks {
            assert!(text.lines().count() <= 1, "should not have multiline strings");
            match ann {
                Some(ann) => s.push_str(&format!("{text:?} -> {ann}\n")),
                None => s.push_str(&format!("{text:?}\n",)),
            }
        }

        expect.assert_eq(&s);
    }

    check::<i32>("", [], expect![""]);

    check(
        "abc",
        [("0:0..0:1", 1), ("0:1..0:2", 2), ("0:2..0:3", 3)],
        expect![[r#"
            "a" -> 1
            "b" -> 2
            "c" -> 3
        "#]],
    );

    // allowed to annotate past the end of the text
    check(
        "abc",
        [("0:0..0:1", 1), ("0:1..0:2", 2), ("0:2..0:4", 3)],
        expect![[r#"
            "a" -> 1
            "b" -> 2
            "c" -> 3
            " " -> 3
        "#]],
    );

    check(
        "1\n2\n3\n",
        [("0:0..0:1", 1), ("1:0..1:1", 2), ("2:0..2:1", 3)],
        expect![[r#"
            "1" -> 1
            "\n"
            "2" -> 2
            "\n"
            "3" -> 3
            "\n"
        "#]],
    );

    check(
        "1\n2\n3\n",
        [("0:0..0:2", 1), ("1:0..1:2", 2), ("2:0..2:3", 3)],
        expect![[r#"
            "1\n" -> 1
            "2\n" -> 2
            "3\n" -> 3
            " " -> 3
        "#]],
    );

    check(
        "1\n2\n3\n",
        [("0:0..0:2", 1), ("1:0..1:3", 2), ("2:0..2:3", 3)],
        expect![[r#"
            "1\n" -> 1
            "2\n" -> 2
            " " -> 2
            "3\n" -> 3
            " " -> 3
        "#]],
    );

    check(
        r#"package main

func main() {}
"#,
        [
            ("0:0..0:7", "keyword|cursorline"),
            ("0:7..0:50", "cursorline"),
            ("2:0..2:4", "keyword"),
            ("2:5..2:9", "function"),
            ("2:5..2:9", "identifier"),
        ],
        expect![[r#"
            "package" -> keyword|cursorline
            " main\n" -> cursorline
            "                                     " -> cursorline
            "\n"
            "func" -> keyword
            " "
            "main" -> function
            "() {}\n"
        "#]],
    );

    check(
        stringify!(
const x: &str = r#"
1
2
"#),
        [("0:0..0:5", "kw"), ("0:16..0:20", "str"), ("0:20..0:28", "cursorline")],
        expect![[r##"
            "const" -> kw
            " x: &str = "
            "r#\"\n" -> str
            "        " -> cursorline
            "1\n"
            "2\n"
            "\"#"
        "##]],
    );
}
