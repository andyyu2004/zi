use super::*;

#[test]
fn line_object() {
    assert_eq!(Line.byte_range(&"\n", 0), 0..1);
    assert_eq!(Line.byte_range(&"\n", 1), 0..1);
}
