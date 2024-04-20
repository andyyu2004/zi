use super::*;

#[test]
fn line_object() {
    assert_eq!(Line(Inclusivity::Inclusive).byte_range(&"\n", 0), Some(0..1));
    assert_eq!(Line(Inclusivity::Inclusive).byte_range(&"\n", 1), Some(0..1));
    assert_eq!(Line(Inclusivity::Exclusive).byte_range(&"\n", 0), Some(0..0),);
    assert_eq!(Line(Inclusivity::Exclusive).byte_range(&"\n", 1), Some(1..1));

    assert_eq!(Line(Inclusivity::Inclusive).byte_range(&"abc\nd", 0), Some(0..4));
    assert_eq!(Line(Inclusivity::Exclusive).byte_range(&"abc\nd", 0), Some(0..3));
}
