use super::*;

#[test]
fn line_object() {
    assert_eq!(Line::inclusive().byte_range(&"\n", 0), Some(0..0));
    assert_eq!(Line::inclusive().byte_range(&"\n", 1), Some(0..0));
    assert_eq!(Line::inclusive().byte_range(&"\n\n", 0), Some(0..1));
    assert_eq!(Line::inclusive().byte_range(&"\n\n", 1), Some(0..1));
    assert_eq!(Line::exclusive().byte_range(&"\n", 0), Some(0..0),);
    assert_eq!(Line::exclusive().byte_range(&"\n", 1), Some(1..1));

    assert_eq!(Line::inclusive().byte_range(&"abc\nd", 0), Some(0..4));
    assert_eq!(Line::exclusive().byte_range(&"abc\nd", 0), Some(0..3));

    assert_eq!(Line::exclusive().byte_range(&" ab", 1), Some(1..3));
    assert_eq!(Line::exclusive().byte_range(&"  ab", 1), Some(2..4));
    assert_eq!(Line::exclusive().byte_range(&"  ab ", 1), Some(2..5));
    assert_eq!(Line::exclusive().byte_range(&"  ab ", 0), Some(2..5));
}
