use std::fmt;

use zi_text::{deltas, MarkTree};

#[track_caller]
fn assert_iter_eq<T: Copy + Eq + fmt::Debug>(
    values: impl Iterator<Item = T>,
    expected: impl IntoIterator<Item = T>,
) {
    let values = values.collect::<Vec<_>>();
    let expected = expected.into_iter().collect::<Vec<_>>();
    assert_eq!(values, expected);
}

#[test]
fn marktree_range_iter() {
    let mut tree = MarkTree::<_, 10>::new(1000);
    tree.insert((0, 0u64));
    tree.insert((1, 1));

    assert_iter_eq(tree.items(0..0), []);
    assert_iter_eq(tree.items(0..=0), [(0, 0)]);
    assert_iter_eq(tree.items(0..1), [(0, 0)]);
    assert_iter_eq(tree.items(0..2), [(0, 0), (1, 1)]);
    assert_iter_eq(tree.items(1..2), [(1, 1)]);
    assert_iter_eq(tree.items(2..2), []);

    (2..100).for_each(|i| tree.insert((i, i as u64)));

    assert_iter_eq(tree.items(0..0), []);
    assert_iter_eq(tree.items(0..1), [(0, 0)]);
    assert_iter_eq(tree.items(0..2), [(0, 0), (1, 1)]);
    assert_iter_eq(tree.items(1..2), [(1, 1)]);
    assert_iter_eq(tree.items(2..2), []);

    assert_iter_eq(tree.items(0..3), [(0, 0), (1, 1), (2, 2)]);
    assert_iter_eq(tree.items(0..=2), [(0, 0), (1, 1), (2, 2)]);
    assert_iter_eq(tree.items(20..40), (20..40).map(|i| (i, i as u64)));
    assert_iter_eq(tree.items(80..100), (80..100).map(|i| (i, i as u64)));
    assert_iter_eq(tree.items(80..=100), (80..100).map(|i| (i, i as u64)));
}

#[test]
fn marktree_delete() {
    let mut tree = MarkTree::<_, 8>::new(10);
    tree.insert((0, 0u64));
    tree.insert((0, 1));

    assert_iter_eq(tree.items(..), [(0, 0), (0, 1)]);

    tree.delete(0);
    assert_iter_eq(tree.items(..), [(0, 1)]);
}

#[test]
fn marktree_clear_range() {
    let mut tree = MarkTree::<_, 10>::new(10);

    tree.insert((0, 0u64));
    assert_iter_eq(tree.items(..), [(0, 0)]);
    assert_eq!(tree.len(), 10);

    tree.clear_range(0..1);
    assert_iter_eq(tree.items(..), []);
    assert_eq!(tree.len(), 10);

    tree.insert((1, 1));
    assert_iter_eq(tree.items(..), [(1, 1)]);
    assert_eq!(tree.len(), 10);

    tree.clear_range(0..1);
    assert_iter_eq(tree.items(..), [(1, 1)]);
    assert_eq!(tree.len(), 10);

    tree.clear_range(0..2);
    assert_iter_eq(tree.items(..), []);
    assert_eq!(tree.len(), 10);
}

#[test]
fn marktree_bulk_remove_range() {
    const LEN: usize = 200;
    let mut tree = MarkTree::<_, 2>::new(LEN);

    (0..100).for_each(|i| tree.insert((i, i as u64)));
    assert_iter_eq(tree.items(..), (0..100).map(|i| (i, i as u64)));
    assert_eq!(tree.len(), LEN);

    tree.clear_range(0..20);
    assert_iter_eq(tree.items(..), (20..100).map(|i| (i, i as u64)));
    assert_eq!(tree.len(), LEN);

    tree.clear_range(80..100);
    assert_iter_eq(tree.items(..), (20..80).map(|i| (i, i as u64)));
    assert_eq!(tree.len(), LEN);
}

#[test]
fn marktree_simple_insert() {
    let mut tree = MarkTree::<_, 10>::new(2);
    tree.insert((1, 0u64));

    assert_iter_eq(tree.items(..), [(1, 0)]);

    tree.insert((1, 1));
    assert_iter_eq(tree.items(..), [(1, 0), (1, 1)]);

    tree.insert((0, 2));
    assert_iter_eq(tree.items(..), [(0, 2), (1, 0), (1, 1)]);
}

#[test]
fn marktree_split() {
    let mut tree = MarkTree::<_, 2>::new(100);
    (0..100).for_each(|i| tree.insert((i, i as u64)));
    assert_iter_eq(tree.items(..), (0..100).map(|i| (i, i as u64)));
}

#[test]
fn marktree_bulk_insert() {
    #[track_caller]
    fn check<const N: usize>() {
        const LEN: usize = 1000;
        let mut tree = MarkTree::<_, N>::new(LEN);
        let k = 500;
        let values = (0..k).map(|i| (i, i as u64)).collect::<Vec<_>>();
        values.iter().for_each(|&i| {
            tree.insert(i);
            assert_eq!(tree.len(), LEN);
            assert_iter_eq(tree.items(..).map(|(a, _)| a), 0..=i.0);
        });
    }

    check::<2>();
    check::<3>();
    check::<4>();
    check::<5>();
    check::<6>();
    check::<7>();
}

#[test]
fn marktree_edit_append() {
    let mut tree = MarkTree::<(usize, u64), 32>::new(0);
    tree.edit(&deltas![0..0 => "a"]);
    assert_iter_eq(tree.items(..), []);
    assert_eq!(tree.len(), 1);
}

#[test]
fn marktree_edit_append_delete() {
    let mut tree = MarkTree::<(usize, u64), 32>::new(0);

    tree.edit(&deltas![0..0 => "a"]);
    assert_iter_eq(tree.items(..), []);
    assert_eq!(tree.len(), 1);

    tree.edit(&deltas![0..1 => ""]);
    assert_iter_eq(tree.items(..), []);
    assert_eq!(tree.len(), 0);
}

#[test]
fn marktree_edit_delete() {
    let mut tree = MarkTree::<(usize, u64), 32>::new(3);
    assert_eq!(tree.len(), 3);
    tree.edit(&deltas![0..3 => "a"]);

    assert_iter_eq(tree.items(..), []);
    assert_eq!(tree.len(), 1);
}

#[test]
fn marktree_edit() {
    let mut tree = MarkTree::<_, 10>::new(10);

    tree.insert((1, 0u64));
    assert_iter_eq(tree.items(..), [(1, 0)]);

    tree.edit(&deltas![0..0 => "ab"]);
    assert_iter_eq(tree.items(..), [(3, 0)]);
    assert_eq!(tree.len(), 12);

    tree.edit(&deltas![0..1 => ""]);
    assert_eq!(tree.len(), 11);
    assert_iter_eq(tree.items(..), [(2, 0)]);
}

#[test]
fn marktree_smoke() {
    let mut tree = MarkTree::<_, 10>::new(10);
    assert_eq!(tree.len(), 10);
    tree.insert((0, 0u64));
    tree.insert((3, 1));

    assert_iter_eq(tree.items(..), [(0, 0), (3, 1)]);

    tree.insert((3, 2));
    assert_iter_eq(tree.items(..), [(0, 0), (3, 1), (3, 2)]);

    tree.insert((2, 4));
    assert_iter_eq(tree.items(..), [(0, 0), (2, 4), (3, 1), (3, 2)]);
    assert_eq!(tree.len(), 10);
}

#[test]
fn marktree_get() {
    let mut tree = MarkTree::<_, 10>::new(10);
    tree.insert((0, 0u64));
    tree.insert((3, 1));
    tree.insert((3, 2));
    tree.insert((2, 4));

    assert_eq!(tree.get(0), Some((0, 0)));
    assert_eq!(tree.get(1), Some((3, 1)));
    assert_eq!(tree.get(2), Some((3, 2)));
    assert_eq!(tree.get(3), None);
    assert_eq!(tree.get(4), Some((2, 4)));
}

#[test]
fn marktree_bulk_get() {
    let mut tree = MarkTree::<_, 8>::new(10000);
    let k = 4000;
    (0..k).for_each(|i| tree.insert((i, i as u64)));
    (0..k).for_each(|i| {
        let item = tree.get(i as u64);
        assert_eq!(item, Some((i, i as u64)));
    });
}
