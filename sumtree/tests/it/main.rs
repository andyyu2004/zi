use sumtree::MarkTree;

fn check_chunks<'a, T: Copy + Eq + 'a>(
    chunks: impl Iterator<Item = &'a [T]> + 'a,
    expected: impl IntoIterator<Item = T>,
) {
    let values = chunks.flat_map(|chunk| chunk.iter().copied());
    assert!(values.eq(expected));
}

#[test]
fn singleton() {
    let mut tree = MarkTree::<_, 2>::default();
    tree.insert(1);

    let mut chunks = tree.chunks();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks.next().unwrap(), [1]);
    assert_eq!(chunks.len(), 0);
    assert_eq!(chunks.next(), None);
}

#[test]
fn smoke() {
    let mut tree = MarkTree::<_, 2>::default();
    tree.insert(1);
    tree.insert(2);

    check_chunks(tree.chunks(), [1, 2]);

    tree.insert(2);
    check_chunks(tree.chunks(), [1, 2]);
}
