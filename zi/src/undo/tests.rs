use super::*;

#[test]
fn test_undo_tree() {
    let mut t = UndoTree::default();
    t.push(0);
    t.push(1);
    assert_eq!(t.undo(), Some(&1));
    assert_eq!(t.undo(), Some(&0));

    assert_eq!(t.redo(), Some(&0));
    assert_eq!(t.redo(), Some(&1));
    assert_eq!(t.redo(), None);
}
