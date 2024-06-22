use sumtree::SumTree;

#[test]
fn smoke() {
    let mut tree = SumTree::<_, 2>::default();
    tree.replace(0..0, 1);

    let mut chunks = tree.chunks();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks.next().unwrap(), [1]);
    assert_eq!(chunks.len(), 0);
    assert_eq!(chunks.next(), None);
}
