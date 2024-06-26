use std::fmt;

use zi_text::{MTree, MarkTree};

#[track_caller]
fn assert_offset_iter_eq<T: Copy + Eq + fmt::Debug + 'static>(
    values: impl Iterator<Item = (usize, T)>,
    expected: impl IntoIterator<Item = (usize, T)>,
) {
    let values = values.collect::<Vec<_>>();
    let expected = expected.into_iter().collect::<Vec<_>>();
    assert_eq!(values, expected);
}

#[track_caller]
fn assert_iter_eq<T: Copy + Eq + fmt::Debug + 'static>(
    values: impl Iterator<Item = T>,
    expected: impl IntoIterator<Item = T>,
) {
    let values = values.collect::<Vec<_>>();
    let expected = expected.into_iter().collect::<Vec<_>>();
    assert_eq!(values, expected);
}

fn new(n: usize) -> MarkTree<u64, 4> {
    MarkTree::new(n)
}

#[test]
fn marktree_empty() {
    let mut tree = new(1);
    // It should be fine to insert at index == tree.len()
    tree.insert(0, 0u64);
    assert_offset_iter_eq(tree.range(..), [(0, 0)]);
    assert_offset_iter_eq(tree.range(..0), []);
    assert_offset_iter_eq(tree.range(..=0), [(0, 0)]);

    tree.shift(0..0, 1);
    assert_offset_iter_eq(tree.range(..), [(1, 0)]);

    assert_eq!(tree.delete(0), Some(1));
    assert_offset_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_range_iter() {
    let mut tree = new(1000);
    tree.insert(0, 0u64);
    tree.insert(1, 1);

    assert_offset_iter_eq(tree.range(0..0), []);
    assert_offset_iter_eq(tree.range(0..=0), [(0, 0)]);
    assert_offset_iter_eq(tree.range(0..1), [(0, 0)]);
    assert_offset_iter_eq(tree.range(0..2), [(0, 0), (1, 1)]);
    assert_offset_iter_eq(tree.range(1..2), [(1, 1)]);
    assert_offset_iter_eq(tree.range(2..2), []);

    (2..100).for_each(|i| tree.insert(i, i as u64));

    assert_offset_iter_eq(tree.range(0..0), []);
    assert_offset_iter_eq(tree.range(0..1), [(0, 0)]);
    assert_offset_iter_eq(tree.range(0..2), [(0, 0), (1, 1)]);
    assert_offset_iter_eq(tree.range(1..2), [(1, 1)]);
    assert_offset_iter_eq(tree.range(2..2), []);

    assert_offset_iter_eq(tree.range(0..3), [(0, 0), (1, 1), (2, 2)]);
    assert_offset_iter_eq(tree.range(0..=2), [(0, 0), (1, 1), (2, 2)]);
    assert_offset_iter_eq(tree.range(20..40), (20..40).map(|i| (i, i as u64)));
    assert_offset_iter_eq(tree.range(80..100), (80..100).map(|i| (i, i as u64)));
    assert_offset_iter_eq(tree.range(80..=100), (80..100).map(|i| (i, i as u64)));
}

#[test]
fn marktree_delete() {
    let mut tree = new(10);
    tree.insert(0, 0u64);
    tree.insert(0, 1);

    assert_offset_iter_eq(tree.range(..), [(0, 0), (0, 1)]);

    tree.delete(0);
    assert_offset_iter_eq(tree.range(..), [(0, 1)]);

    tree.delete(1);
    assert_offset_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_bulk_delete() {
    let mut tree = new(10000);
    let k = 2000u64;
    (0..k).for_each(|i| tree.insert(i as usize, i));
    (0..k).for_each(|i| {
        assert_eq!(tree.delete(i), Some(i as usize));
        assert_offset_iter_eq(tree.range(..), (i + 1..k).map(|j| (j as usize, j)));
    });
}

#[test]
fn marktree_drain_2() {
    let mut tree = new(10);
    (0..4).for_each(|i| tree.insert(i, i as u64));

    assert_iter_eq(tree.drain(0..=0), [(0, 0)]);
    assert_offset_iter_eq(tree.range(..), [(1, 1), (2, 2), (3, 3)]);

    assert_iter_eq(tree.drain(1..=1), [(1, 1)]);
    assert_offset_iter_eq(tree.range(..), [(2, 2), (3, 3)]);

    tree.drain(2..=2);
    assert_offset_iter_eq(tree.range(..), [(3, 3)]);

    tree.drain(3..=3);
    assert_offset_iter_eq(tree.range(..), []);

    tree.insert(0, 0);
    tree.insert(0, 1);
    assert_offset_iter_eq(tree.range(..), [(0, 0), (0, 1)]);

    tree.drain(0..0);
    assert_offset_iter_eq(tree.range(..), [(0, 0), (0, 1)]);

    tree.drain(0..1);
    assert_offset_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_drain() {
    let mut tree = new(10);

    tree.insert(0, 0u64);
    assert_offset_iter_eq(tree.range(..), [(0, 0)]);
    assert_eq!(tree.len(), 10);

    assert_iter_eq(tree.drain(0..1), [(0, 0)]);
    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 10);

    tree.insert(1, 1);
    assert_offset_iter_eq(tree.range(..), [(1, 1)]);
    assert_eq!(tree.len(), 10);

    assert_iter_eq(tree.drain(0..1), []);
    assert_offset_iter_eq(tree.range(..), [(1, 1)]);
    assert_eq!(tree.len(), 10);

    assert_iter_eq(tree.drain(0..2), [(1, 1)]);
    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 10);
}

#[test]
fn marktree_bulk_drain() {
    const LEN: usize = 200;
    let mut tree = new(LEN);

    (0..100).for_each(|i| tree.insert(i, i as u64));
    assert_offset_iter_eq(tree.range(..), (0..100).map(|i| (i, i as u64)));
    assert_eq!(tree.len(), LEN);

    tree.drain(0..20);
    assert_offset_iter_eq(tree.range(..), (20..100).map(|i| (i, i as u64)));
    assert_eq!(tree.len(), LEN);

    tree.drain(80..100);
    assert_offset_iter_eq(tree.range(..), (20..80).map(|i| (i, i as u64)));
    assert_eq!(tree.len(), LEN);
}

#[test]
fn marktree_simple_insert() {
    let mut tree = new(2);
    tree.insert(1, 0u64);

    assert_offset_iter_eq(tree.range(..), [(1, 0)]);

    tree.insert(1, 1);
    assert_offset_iter_eq(tree.range(..), [(1, 0), (1, 1)]);

    tree.insert(0, 2);
    assert_offset_iter_eq(tree.range(..), [(0, 2), (1, 0), (1, 1)]);
}

#[test]
fn marktree_split() {
    let mut tree = new(100);
    (0..100).for_each(|i| tree.insert(i, i as u64));
    assert_offset_iter_eq(tree.range(..), (0..100).map(|i| (i, i as u64)));
}

#[test]
fn marktree_bulk_insert() {
    #[track_caller]
    fn check<const N: usize>() {
        const LEN: usize = 1000;
        let mut tree = new(LEN);
        let k = 500;
        let values = (0..k).map(|i| (i, i as u64)).collect::<Vec<_>>();
        values.iter().for_each(|&(at, i)| {
            tree.insert(at, i);
            assert_eq!(tree.len(), LEN);
            assert_iter_eq(tree.range(..).map(|(a, _)| a), 0..=at);
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
fn marktree_shift_append() {
    let mut tree = new(0);
    tree.shift(0..0, 1);
    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 1);
}

#[test]
fn marktree_shift_append_delete() {
    let mut tree = new(0);

    tree.shift(0..0, 1);
    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 1);

    tree.shift(0..1, 0);
    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 0);
}

#[test]
fn marktree_shift_delete() {
    let mut tree = new(3);
    assert_eq!(tree.len(), 3);
    tree.shift(0..3, 1);

    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 1);
}

#[test]
fn marktree_shift() {
    let mut tree = new(10);

    tree.insert(1, 0u64);
    assert_offset_iter_eq(tree.range(..), [(1, 0)]);

    tree.shift(0..0, 2);
    assert_offset_iter_eq(tree.range(..), [(3, 0)]);
    assert_eq!(tree.len(), 12);

    tree.shift(0..1, 0);
    assert_eq!(tree.len(), 11);
    assert_offset_iter_eq(tree.range(..), [(2, 0)]);
}

#[test]
fn marktree_smoke() {
    let mut tree = new(10);
    assert_eq!(tree.len(), 10);
    tree.insert(0, 0u64);
    tree.insert(3, 1);

    assert_offset_iter_eq(tree.range(..), [(0, 0), (3, 1)]);

    tree.insert(3, 2);
    assert_offset_iter_eq(tree.range(..), [(0, 0), (3, 1), (3, 2)]);

    tree.insert(2, 4);
    assert_offset_iter_eq(tree.range(..), [(0, 0), (2, 4), (3, 1), (3, 2)]);
    assert_eq!(tree.len(), 10);
}

#[test]
fn marktree_get() {
    let mut tree = new(10);
    tree.insert(0, 0u64);
    tree.insert(3, 1);
    tree.insert(3, 2);
    tree.insert(2, 4);

    assert_eq!(tree.get(0), Some(0));
    assert_eq!(tree.get(1), Some(3));
    assert_eq!(tree.get(2), Some(3));
    assert_eq!(tree.get(3), None);
    assert_eq!(tree.get(4), Some(2));
}

#[test]
fn marktree_bulk_get() {
    let mut tree = new(10000);
    let k = 4000;
    (0..k).for_each(|i| tree.insert(i, i as u64));
    (0..k).for_each(|i| {
        let offset = tree.get(i as u64);
        assert_eq!(offset, Some(i));
    });
}
