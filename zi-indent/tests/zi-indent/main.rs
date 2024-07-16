use zi_indent::{indent, Config, Indent, Text};

#[track_caller]
fn check(text: impl Text, line_idx: usize, expected: Indent) {
    assert_eq!(indent(Config::default(), &text, line_idx), expected);
}

#[test]
fn indent_smoke() {
    check(r#""#, 0, Indent::Bytes(0));

    check(
        r#"fn main() {
    let foo = 42;
}"#,
        2,
        Indent::Bytes(4),
    );

    check(
        r#"fn main() {
"#,
        1,
        Indent::Bytes(4),
    );
}
