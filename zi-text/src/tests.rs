use expect_test::{expect, Expect};
use proptest::collection::vec;
use proptest::strategy::{Just, Strategy};
use proptest::{bool, proptest};

use super::*;

fn mut_impls<'a>(s: &'a str) -> [Box<dyn AnyTextMut + 'a>; 2] {
    [
        // could use crop::Rope::from directly, but using the building is more realistic
        Box::new({
            let mut builder = crop::RopeBuilder::new();
            builder.append(s);
            builder.build()
        }) as Box<dyn AnyTextMut>,
        Box::new(s.to_owned()),
    ]
}

#[test]
fn text_edit() {
    for mut imp in mut_impls("abc") {
        imp.edit(&Delta::new(0..0, "x"));
        assert_eq!(imp.to_string(), "xabc");
    }
}

fn impls<'a>(s: &'a str) -> [Box<dyn AnyText + 'a>; 3] {
    [
        // could use crop::Rope::from directly, but using the building is more realistic
        Box::new({
            let mut builder = crop::RopeBuilder::new();
            builder.append(s);
            builder.build()
        }) as Box<dyn AnyText>,
        Box::new(ReadonlyText::new(s.as_bytes())),
        Box::new(s),
    ]
}

#[test]
fn chars_at() {
    for imp in impls("abc") {
        let mut chars = imp.byte_slice(..3).chars().rev();
        assert_eq!(chars.next(), Some('c'));
    }
}

#[test]
fn char_at_byte() {
    assert_eq!("abc".char_at_byte(0), Some('a'));
    assert_eq!("abc".char_at_byte(1), Some('b'));
    assert_eq!("abc".char_at_byte(2), Some('c'));
    assert_eq!("abc".char_at_byte(3), None);
    assert_eq!("ab©d".char_at_byte(2), Some('©'));
    assert_eq!("ab©d".char_at_byte(4), Some('d'));
}

#[test]
fn empty_text() {
    macro_rules! test {
        ($text:expr) => {
            let text = $text;
            assert_eq!(text.len_bytes(), 0, "len_bytes");
            assert_eq!(text.len_lines(), 0, "len_lines");
            assert_eq!(text.lines().count(), 0, "lines().count()");
            assert!(text.get_line(0).is_none(), "get_line(0)");
        };
    }

    for imp in impls("") {
        test!(&imp);
        test!(imp.byte_slice(..));
        test!(imp.line_slice(..));
    }
}

// return ascii only so any byte range is valid
fn ascii_str_and_range() -> impl Strategy<Value = (String, ops::Range<usize>)> {
    // using a prefix character x and + to ensure string of length at least 2
    "x[ -~]+"
        .prop_flat_map(|vec| {
            let len = vec.len();
            // minus 1 here to ensure the next step will pass a non-empty range
            (Just(vec), 0..len - 1)
        })
        .prop_flat_map(|(vec, start)| {
            let end = vec.len();
            assert!(start < end, "proptest requires non-empty range");
            (Just(vec), Just(start), start..end)
        })
        .prop_map(|(vec, start, end)| (vec, start..end))
}

proptest! {
    #[test]
    fn invert((mut s, range) in ascii_str_and_range(), replacement in "[ -~]*") {
        let delta = Delta::new(range, replacement);
        let original = s.clone();
        let invert = delta.apply(&mut s);
        invert.apply(&mut s);
        assert_eq!(s, original, "applying the inverse delta should result in the original text");

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

fn test(s: &str) {
    // Test against the rope implementation as that one is probably correct
    // TODO add more test cases and we're not testing the slice impls
    let rope = crop::Rope::from(s);
    let reference = &rope as &dyn AnyText;

    for imp in impls(s) {
        let line_slice = imp.line_slice(..);
        let byte_slice = imp.line_slice(..);

        assert_eq!(reference.len_bytes(), imp.len_bytes());
        assert_eq!(reference.len_lines(), line_slice.len_lines());
        assert_eq!(reference.len_lines(), byte_slice.len_lines());

        assert_eq!(reference.len_lines(), imp.len_lines());
        assert_eq!(reference.len_lines(), line_slice.len_lines());
        assert_eq!(reference.len_lines(), byte_slice.len_lines());

        // check that the line length is self consistent
        assert_eq!(reference.len_lines(), reference.lines().count());
        assert_eq!(imp.len_lines(), imp.lines().count());

        let mut b = 0;
        for c in reference.chars() {
            assert_eq!(reference.byte_to_line(b), imp.byte_to_line(b), "{s:?}: byte {b}");
            assert_eq!(reference.byte_to_line(b), line_slice.byte_to_line(b), "{s:?}: byte {b}");
            assert_eq!(reference.byte_to_line(b), byte_slice.byte_to_line(b), "{s:?}: byte {b}");

            assert_eq!(reference.byte_to_point(b), imp.byte_to_point(b), "{s:?}: byte {b}");
            assert_eq!(reference.byte_to_point(b), line_slice.byte_to_point(b), "{s:?}: byte {b}");
            assert_eq!(reference.byte_to_point(b), byte_slice.byte_to_point(b), "{s:?}: byte {b}");

            b += c.len_utf8();
        }

        // should work when the byte is at the end of the string
        assert_eq!(reference.byte_to_line(b), imp.byte_to_line(b), "{s:?}: byte {b}");
        assert_eq!(reference.byte_to_line(b), line_slice.byte_to_line(b), "{s:?}: byte {b}");
        assert_eq!(reference.byte_to_line(b), byte_slice.byte_to_line(b), "{s:?}: byte {b}");

        assert_eq!(reference.byte_to_point(b), imp.byte_to_point(b), "{s:?}: byte {b}");
        assert_eq!(reference.byte_to_point(b), line_slice.byte_to_point(b), "{s:?}: byte {b}");
        assert_eq!(reference.byte_to_point(b), byte_slice.byte_to_point(b), "{s:?}: byte {b}");

        for l in 0..=reference.len_lines() {
            assert_eq!(
                reference.get_line(l).map(|s| s.to_string()),
                imp.get_line(l).map(|s| s.to_string()),
                "{s:?}: on line {l}"
            );
            assert_eq!(reference.line_to_byte(l), imp.line_to_byte(l), "{s:?}`: on line {l}");
            assert_eq!(
                reference.try_line_to_byte(l),
                imp.try_line_to_byte(l),
                "{s:?}`: on line {l}"
            );
        }

        assert!(reference.lines().map(|s| s.to_string()).eq(imp.lines().map(|s| s.to_string())));
    }
}

proptest! {
    // Ignore some annoying control characters like vertical tabs, nextline etc. No idea if anyone actually uses that in practice.
    // Also skipping \r as usually it's followed by \n.
    #[test]
    fn text_impls(s in "[^\r\u{b}\u{c}\u{85}\u{2028}\u{2029}]*") {
        test(&s)
    }

    #[test]
    fn text_impls_ascii(s in "[ -~]*") {
        test(&s)
    }
}

#[test]
fn byte_to_line() {
    #[track_caller]
    fn check(s: &str, byte: usize) {
        let reference = crop::Rope::from(s).byte_to_line(byte);
        for imp in impls(s) {
            assert_eq!(imp.byte_to_line(byte), reference, "{s:?}: byte {byte}");
        }
    }

    check("ab\n\n", 4);
    check("ab\n", 3);
    check("ab\nc", 4);
    check("ab", 2);
}

#[test]
fn try_line_to_byte() {
    #[track_caller]
    fn check(s: &str, line: usize) {
        let reference = crop::Rope::from(s).try_line_to_byte(line);
        for imp in impls(s) {
            assert_eq!(imp.try_line_to_byte(line), reference);
        }
    }

    check("\n\n\n", 2);
    check("\n\n\n", 3);
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
