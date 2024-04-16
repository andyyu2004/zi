use super::*;

#[test]
fn line_object() {
    assert_eq!(Line.byte_range(&"\n", 0), Ok(0..1));
    assert_eq!(Line.byte_range(&"\n", 1), Ok(0..1));
}
