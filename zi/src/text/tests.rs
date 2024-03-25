use expect_test::{expect, Expect};
use proptest::collection::vec;
use proptest::{bool, proptest};

use super::*;

fn impls<'a>(s: &'a str) -> [Box<dyn AnyText + 'a>; 3] {
    [
        Box::new(crop::Rope::from(s)) as Box<dyn AnyText>,
        Box::new(ReadonlyText::new(s.as_bytes())),
        Box::new(s),
    ]
}

#[test]
fn empty_text() {
    let reference = crop::Rope::from("");
    for imp in impls("") {
        assert_eq!(reference.len_bytes(), imp.len_bytes());
        assert_eq!(reference.len_lines(), imp.len_lines());
        assert_eq!(reference.lines().count(), imp.lines().count());
    }
}

proptest! {
    #[test]
    fn text_chars(s in "[^\r\u{b}\u{c}\u{85}\u{2028}\u{2029}]*", steps in vec(bool::ANY, 1..100)) {
        let reference = crop::Rope::from(s.as_ref());
        for imp in impls(&s) {
            let mut chars = reference.chars();
            let mut imp_chars = imp.chars();

            for &step in &steps {
                if step {
                    assert_eq!(chars.next(), imp_chars.next());
                } else {
                    assert_eq!(chars.next_back(), imp_chars.next_back());
                }
            }
        }
    }
}

proptest! {
    // Ignore some annoying control characters like vertical tabs, nextline etc. No idea if anyone actually uses that in practice.
    // Also skipping \r as usually it's followed by \n.
    #[test]
    fn text_impls(s in "[^\r\u{b}\u{c}\u{85}\u{2028}\u{2029}]*") {
        // Test against the rope implementation as that one is probably correct
        // TODO add more test cases and we're not testing the slice impls

        let rope = crop::Rope::from(s.as_ref());
        let reference = &rope as &dyn AnyText;

        for imp in impls(&s) {
            assert_eq!(reference.len_bytes(), imp.len_bytes());
            assert_eq!(reference.len_lines(), imp.len_lines());

            // check that the line length is self consistent
            assert_eq!(reference.len_lines(), reference.lines().count());
            assert_eq!(imp.len_lines(), imp.lines().count());

            let mut b = 0;
            for c in reference.chars() {
                assert_eq!(reference.byte_to_line(b), imp.byte_to_line(b), "{s:?}: byte {b}");
                assert_eq!(reference.byte_to_point(b), imp.byte_to_point(b), "{s:?}: byte {b}");
                b += c.len_utf8();
            }

            for l in 0..reference.len_lines() {
                assert_eq!(reference.get_line(l).map(|s| s.to_string()), imp.get_line(l).map(|s| s.to_string()), "{s:?}: on line {l}");
                assert_eq!(reference.line_to_byte(l), imp.line_to_byte(l), "{s:?}`: on line {l}");
            }

            assert!(reference.lines().map(|s| s.to_string()).eq(imp.lines().map(|s| s.to_string())));

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
            assert!(text.lines().count() <= 2, "should not have multiline strings: got {text:?}");
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
            "\n"
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
            "\n"
        "#]],
    );

    // indexed by byte, not char
    check(
        "ab©d",
        [("0:0..0:2", 'x'), ("0:2..0:4", 'y'), ("0:4..0:5", 'z')],
        expect![[r#"
            "ab" -> x
            "©" -> y
            "d" -> z
            "\n"
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
        [("0:0..0:1", 1), ("1:0..1:1", 2), ("2:0..2:3", 3)],
        expect![[r#"
            "1" -> 1
            "\n"
            "2" -> 2
            "\n"
            "3" -> 3
            "  " -> 3
            "\n"
        "#]],
    );

    check(
        "1\n2\n3\n",
        [("0:0..0:2", 1), ("1:0..1:3", 2), ("2:0..2:3", 3)],
        expect![[r#"
            "1" -> 1
            " " -> 1
            "\n"
            "2" -> 2
            "  " -> 2
            "\n"
            "3" -> 3
            "  " -> 3
            "\n"
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
            " main" -> cursorline
            "                                      " -> cursorline
            "\n"
            "\n"
            "func" -> keyword
            " "
            "main" -> function
            "() {}"
            "\n"
        "#]],
    );

    check(
        stringify!(
const x: &str = r#"
1
2
"#),
        [("0:0..0:5", "kw"), ("0:16..0:19", "str"), ("0:19..0:28", "cursorline")],
        expect![[r##"
            "const" -> kw
            " x: &str = "
            "r#\"" -> str
            "         " -> cursorline
            "\n"
            "1"
            "\n"
            "2"
            "\n"
            "\"#"
            "\n"
        "##]],
    );
}
