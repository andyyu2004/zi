use std::fmt;

use zi_text::{deltas, MarkTree};

#[track_caller]
fn check<T: Copy + Eq + fmt::Debug>(
    values: impl Iterator<Item = T>,
    expected: impl IntoIterator<Item = T>,
) {
    let values = values.collect::<Vec<_>>();
    let expected = expected.into_iter().collect::<Vec<_>>();
    assert_eq!(values, expected);
}

#[test]
fn simple_insert() {
    let mut tree = MarkTree::<_, 10>::new(2);
    tree.insert(1);

    check(tree.iter(), [1]);

    tree.insert(1);
    check(tree.iter(), [1, 1]);

    tree.insert(0);
    check(tree.iter(), [0, 1, 1]);
}

#[test]
fn split() {
    let mut tree = MarkTree::<_, 2>::new(100);
    (0..100).for_each(|i| tree.insert(i));
    check(tree.iter(), 0..100);
}

#[test]
fn shift() {
    let mut tree = MarkTree::<_, 10>::new(10);
    tree.insert(1);
    check(tree.iter(), [1]);

    tree.edit(&deltas![0..0 => "ab"]);
    check(tree.iter(), [3]);
}

#[test]
fn smoke() {
    let mut tree = MarkTree::<_, 10>::new(10);
    tree.insert(0);
    tree.insert(3);

    check(tree.iter(), [0, 3]);

    tree.insert(3);
    check(tree.iter(), [0, 3, 3]);

    tree.insert(2);
    check(tree.iter(), [0, 2, 3, 3]);
}
