use super::*;

#[test]
fn line_object() {
    assert_eq!(Line(Inclusivity::Inclusive).byte_range(&"\n", 0), Ok(0..1));
    assert_eq!(Line(Inclusivity::Inclusive).byte_range(&"\n", 1), Ok(0..1));
    assert_eq!(
        Line(Inclusivity::Exclusive).byte_range(&"\n", 0),
        Ok(0..1),
        "newline is included if the byte is exactly on it"
    );
    assert_eq!(Line(Inclusivity::Exclusive).byte_range(&"\n", 1), Ok(1..1));

    assert_eq!(Line(Inclusivity::Inclusive).byte_range(&"abc\nd", 0), Ok(0..4));
    assert_eq!(Line(Inclusivity::Exclusive).byte_range(&"abc\nd", 0), Ok(0..3));
}
