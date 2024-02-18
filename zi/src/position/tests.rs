use super::Position;

#[test]
fn position_coords() {
    assert_eq!(Position::default().coords(), (0, 0));
}
