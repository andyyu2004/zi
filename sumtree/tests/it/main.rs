use sumtree::SumTree;

#[test]
fn marktree() {
    let mut tree = SumTree::<2, _>::default();
    tree.replace(0..0, 1);

    let mut chunks = tree.chunks();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks.next().unwrap(), [1]);
    assert_eq!(chunks.len(), 0);
    assert_eq!(chunks.next(), None);
}
