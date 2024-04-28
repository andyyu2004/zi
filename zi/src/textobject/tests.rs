use super::*;

#[track_caller]
fn check_range(
    obj: &impl TextObject,
    text: &str,
    byte: usize,
    expected: Option<ops::Range<usize>>,
) {
    assert_eq!(obj.byte_range(&text, byte), expected);
    assert_eq!(obj.byte_range(&crop::Rope::from(text), byte), expected);
}

#[test]
fn line_object() {
    let inc = Line::inclusive();
    let exl = Line::exclusive();

    check_range(&inc, "\n", 0, Some(0..1));
    check_range(&inc, "\n", 1, Some(1..1));

    check_range(&inc, "\n\n", 0, Some(0..1));
    check_range(&inc, "\n\n", 1, Some(1..2));

    check_range(&exl, "\n", 0, Some(0..0));
    check_range(&exl, "\n", 1, Some(1..1));

    check_range(&inc, "abc\nd", 0, Some(0..4));
    check_range(&exl, "abc\nd", 0, Some(0..3));

    check_range(&exl, " ab", 1, Some(1..3));
    check_range(&exl, "  ab", 1, Some(2..4));
    check_range(&exl, "  ab ", 1, Some(2..5));
    check_range(&exl, "  ab ", 0, Some(2..5));

    check_range(&inc, "\n\n\n", 2, Some(2..3));
}
