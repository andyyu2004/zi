use std::fmt;

use zi_text::{Bias, MarkTree, MarkTreeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Id(usize);

impl From<Id> for u64 {
    #[inline]
    fn from(id: Id) -> u64 {
        id.0 as u64
    }
}

impl From<u64> for Id {
    #[inline]
    fn from(id: u64) -> Id {
        Id(id as usize)
    }
}

impl MarkTreeId for Id {}

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

fn new(n: usize) -> MarkTree<Id, 4> {
    MarkTree::new(n)
}

#[test]
fn marktree_empty() {
    let mut tree = new(1);
    tree.insert(0, Id(0));
    assert_offset_iter_eq(tree.range(..), [(0, Id(0))]);
    assert_offset_iter_eq(tree.range(..0), []);
    assert_offset_iter_eq(tree.range(..=0), [(0, Id(0))]);

    // Should shift to the right as right-biased by default.
    tree.shift(0..0, 1);
    assert_offset_iter_eq(tree.range(..), [(1, Id(0))]);

    assert_eq!(tree.delete(0), Some(1));
    assert_offset_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_range_iter() {
    let mut tree = new(1000);
    tree.insert(0, Id(0));
    tree.insert(1, Id(1));

    assert_offset_iter_eq(tree.range(0..0), []);
    assert_offset_iter_eq(tree.range(0..=0), [(0, Id(0))]);
    assert_offset_iter_eq(tree.range(0..1), [(0, Id(0))]);
    assert_offset_iter_eq(tree.range(0..2), [(0, Id(0)), (1, Id(1))]);
    assert_offset_iter_eq(tree.range(1..2), [(1, Id(1))]);
    assert_offset_iter_eq(tree.range(2..2), []);

    (2..100).for_each(|i| drop(tree.insert(i, Id(i))));

    assert_offset_iter_eq(tree.range(0..0), []);
    assert_offset_iter_eq(tree.range(0..1), [(0, Id(0))]);
    assert_offset_iter_eq(tree.range(0..2), [(0, Id(0)), (1, Id(1))]);
    assert_offset_iter_eq(tree.range(1..2), [(1, Id(1))]);
    assert_offset_iter_eq(tree.range(2..2), []);

    assert_offset_iter_eq(tree.range(0..3), [(0, Id(0)), (1, Id(1)), (2, Id(2))]);
    assert_offset_iter_eq(tree.range(0..=2), [(0, Id(0)), (1, Id(1)), (2, Id(2))]);
    assert_offset_iter_eq(tree.range(20..40), (20..40).map(|i| (i, Id(i))));
    assert_offset_iter_eq(tree.range(80..100), (80..100).map(|i| (i, Id(i))));
    assert_offset_iter_eq(tree.range(80..=100), (80..100).map(|i| (i, Id(i))));
}

#[test]
fn marktree_id_not_too_large() {
    let mut tree = new(10);
    tree.insert(0, Id(1 << 47));
}

#[test]
#[should_panic]
fn marktree_id_too_large() {
    let mut tree = new(10);
    tree.insert(0, Id(1 << 48));
}

#[test]
fn marktree_delete() {
    let mut tree = new(10);
    tree.insert(0, Id(0));
    tree.insert(0, 1);

    assert_offset_iter_eq(tree.range(..), [(0, Id(0)), (0, Id(1))]);

    tree.delete(0);
    assert_offset_iter_eq(tree.range(..), [(0, Id(1))]);

    tree.delete(1);
    assert_offset_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_bulk_delete() {
    let mut tree = new(10000);
    let k = 2000;
    (0..k).for_each(|i| drop(tree.insert(i, Id(i))));
    (0..k).for_each(|i| {
        assert_eq!(tree.delete(Id(i)), Some(i));
        assert_offset_iter_eq(tree.range(..), (i + 1..k).map(|j| (j, Id(j))));
    });
}

#[test]
fn marktree_drain_2() {
    let mut tree = new(10);
    (0..4).for_each(|i| drop(tree.insert(i, Id(i))));

    assert_iter_eq(tree.drain(0..=0), [(0, Id(0))]);
    assert_offset_iter_eq(tree.range(..), [(1, Id(1)), (2, Id(2)), (3, Id(3))]);

    assert_iter_eq(tree.drain(1..=1), [(1, Id(1))]);
    assert_offset_iter_eq(tree.range(..), [(2, Id(2)), (3, Id(3))]);

    tree.drain(2..=2);
    assert_offset_iter_eq(tree.range(..), [(3, Id(3))]);

    tree.drain(3..=3);
    assert_offset_iter_eq(tree.range(..), []);

    tree.insert(0, 0);
    tree.insert(0, 1);
    assert_offset_iter_eq(tree.range(..), [(0, Id(0)), (0, Id(1))]);

    tree.drain(0..0);
    assert_offset_iter_eq(tree.range(..), [(0, Id(0)), (0, Id(1))]);

    tree.drain(0..1);
    assert_offset_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_drain_1() {
    let mut tree = new(10);

    tree.insert(0, Id(0));
    assert_offset_iter_eq(tree.range(..), [(0, Id(0))]);
    assert_eq!(tree.len(), 10);

    assert_iter_eq(tree.drain(0..1), [(0, Id(0))]);
    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 10);

    tree.insert(1, 1);
    assert_offset_iter_eq(tree.range(..), [(1, Id(1))]);
    assert_eq!(tree.len(), 10);

    assert_iter_eq(tree.drain(0..1), []);
    assert_offset_iter_eq(tree.range(..), [(1, Id(1))]);
    assert_eq!(tree.len(), 10);

    assert_iter_eq(tree.drain(0..2), [(1, Id(1))]);
    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 10);
}

#[test]
fn marktree_bulk_drain() {
    const LEN: usize = 200;
    let mut tree = new(LEN);

    (0..100).for_each(|i| drop(tree.insert(i, Id(i))));
    assert_offset_iter_eq(tree.range(..), (0..100).map(|i| (i, Id(i))));
    assert_eq!(tree.len(), LEN);

    tree.drain(0..20);
    assert_offset_iter_eq(tree.range(..), (20..100).map(|i| (i, Id(i))));
    assert_eq!(tree.len(), LEN);

    tree.drain(80..100);
    assert_offset_iter_eq(tree.range(..), (20..80).map(|i| (i, Id(i))));
    assert_eq!(tree.len(), LEN);
}

#[test]
fn marktree_simple_insert() {
    let mut tree = new(2);
    tree.insert(1, Id(0));

    assert_offset_iter_eq(tree.range(..), [(1, Id(0))]);

    tree.insert(1, 1);
    assert_offset_iter_eq(tree.range(..), [(1, Id(0)), (1, Id(1))]);

    tree.insert(0, 2);
    assert_offset_iter_eq(tree.range(..), [(0, Id(2)), (1, Id(0)), (1, Id(1))]);
}

#[test]
fn marktree_small_insert() {
    let mut tree = new(5);
    (0..5).for_each(|i| {
        tree.insert(i, Id(i));
        assert_iter_eq(tree.range(..), (0..=i).map(|j| (j, Id(j))));
    });
}

#[test]
fn marktree_split() {
    let mut tree = new(100);
    (0..100).for_each(|i| drop(tree.insert(i, Id(i))));
    assert_offset_iter_eq(tree.range(..), (0..100).map(|i| (i, Id(i))));
}

#[test]
fn marktree_bulk_insert() {
    #[track_caller]
    fn check<const N: usize>() {
        const LEN: usize = 1000;
        let mut tree = new(LEN);
        let k = 500;
        let values = (0..k).map(|i| (i, Id(i))).collect::<Vec<_>>();
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
    let mut tree = new(1);
    tree.shift(0..0, 1);
    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 2);
}

#[test]
fn marktree_shift_append_delete() {
    let mut tree = new(1);

    tree.shift(0..0, 1);
    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 2);

    dbg!(&tree);
    tree.shift(0..1, 0);
    dbg!(&tree);
    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 1);
}

// TODO test lots of shift deletions

#[test]
fn marktree_shift_delete() {
    let mut tree = new(3);
    assert_eq!(tree.len(), 3);
    tree.shift(0..3, 1);

    assert_offset_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 1);
}

#[test]
fn marktree_shift_empty() {
    let mut tree = new(5);
    assert_eq!(tree.len(), 5);

    tree.shift(0..1, 0);
    assert_eq!(tree.len(), 4);

    dbg!(&tree);
    tree.shift(1..1, 1);
    dbg!(&tree);
    assert_eq!(tree.len(), 5);
}

#[test]
fn marktree_shift() {
    let mut tree = new(10);

    tree.insert(1, Id(0));
    assert_offset_iter_eq(tree.range(..), [(1, Id(0))]);

    dbg!(&tree);
    tree.shift(0..0, 2);

    dbg!(&tree);
    assert_offset_iter_eq(tree.range(..), [(3, Id(0))]);
    assert_eq!(tree.len(), 12);

    tree.shift(0..1, 0);
    assert_eq!(tree.len(), 11);
    assert_offset_iter_eq(tree.range(..), [(2, Id(0))]);
}

#[test]
fn marktree_smoke() {
    let mut tree = new(10);
    assert_eq!(tree.len(), 10);
    tree.insert(0, Id(0));
    tree.insert(3, Id(1));

    assert_offset_iter_eq(tree.range(..), [(0, Id(0)), (3, Id(1))]);

    tree.insert(3, Id(2));
    assert_offset_iter_eq(tree.range(..), [(0, Id(0)), (3, Id(1)), (3, Id(2))]);

    tree.insert(2, Id(4));
    assert_offset_iter_eq(tree.range(..), [(0, Id(0)), (2, Id(4)), (3, Id(1)), (3, Id(2))]);
    assert_eq!(tree.len(), 10);
}

#[test]
fn marktree_get() {
    let mut tree = new(10);
    assert_eq!(tree.len(), 10);

    tree.insert(0, Id(0));
    assert_eq!(tree.get(0), Some(0));

    tree.insert(3, 1);
    assert_eq!(tree.get(1), Some(3));

    tree.insert(3, 2);
    assert_eq!(tree.get(2), Some(3));

    tree.insert(2, 4);
    assert_eq!(tree.get(4), Some(2));

    assert_eq!(tree.get(0), Some(0));
    assert_eq!(tree.get(1), Some(3));
    assert_eq!(tree.get(2), Some(3));
    assert_eq!(tree.get(3), None);
    assert_eq!(tree.get(4), Some(2));
}

#[test]
fn marktree_duplicate_offsets() {
    let mut tree = new(10);
    (0..1000).for_each(|i| drop(tree.insert(0, i)));
    assert_offset_iter_eq(tree.range(..), (0..1000).map(|i| (0, Id(i))));
    assert_offset_iter_eq(tree.drain(0..=0), (0..1000).map(|i| (0, Id(i))));
    assert_offset_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_bulk_get() {
    let mut tree = new(10000);
    let k = 4000;
    (0..k).for_each(|i| drop(tree.insert(i, Id(i))));
    (0..k).for_each(|i| {
        let offset = tree.get(Id(i));
        assert_eq!(offset, Some(i));
    });
}

#[test]
fn marktree_left_bias() {
    let mut tree = new(10);
    let k = 5;
    (0..k).for_each(|i| drop(tree.insert(i, Id(i)).bias(Bias::Left)));

    assert_offset_iter_eq(tree.range(..), (0..k).map(|i| (i, Id(i))));
}
