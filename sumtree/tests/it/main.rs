use std::fmt;

use sumtree::MarkTree;

#[track_caller]
fn check<'a, T: Copy + Eq + fmt::Debug + 'a>(
    values: impl Iterator<Item = &'a T> + 'a,
    expected: impl IntoIterator<Item = T>,
) {
    let values = values.copied().collect::<Vec<_>>();
    let expected = expected.into_iter().collect::<Vec<_>>();
    assert_eq!(values, expected);
}

#[test]
fn singleton() {
    let mut tree = MarkTree::<_, 10>::new(2);
    tree.insert(0);

    check(tree.iter(), [0]);
}

#[test]
fn smoke() {
    let mut tree = MarkTree::<_, 10>::new(10);
    tree.insert(0);
    tree.insert(3);

    check(tree.iter(), [0, 3]);

    // not sure about dups?
    tree.insert(3);
    check(tree.iter(), [0, 3, 3]);

    tree.insert(2);
    check(tree.iter(), [0, 2, 3, 3]);
}
