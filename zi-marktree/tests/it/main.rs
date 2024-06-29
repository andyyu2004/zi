#![feature(anonymous_lifetime_in_impl_trait)]
use std::ops::Range;
use std::{fmt, iter};

use proptest::collection::vec;
use zi_marktree::{Bias, Inserter, MarkTree, MarkTreeId};

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
fn assert_iter_eq<T: Eq + fmt::Debug + 'static>(
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
    assert_iter_eq(tree.range(..), [(0..0, Id(0))]);
    assert_iter_eq(tree.range(..0), []);
    assert_iter_eq(tree.range(..=0), [(0..0, Id(0))]);

    // Should shift to the right as right-biased by default.
    tree.shift(0..0, 1);
    assert_iter_eq(tree.range(..), [(1..1, Id(0))]);

    assert_eq!(tree.delete(0), Some(1..1));
    assert_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_range_iter() {
    let mut tree = new(1000);
    tree.insert(0, Id(0));
    tree.insert(1, Id(1));

    assert_iter_eq(tree.range(0..0), []);
    assert_iter_eq(tree.range(0..=0), [(0..0, Id(0))]);
    assert_iter_eq(tree.range(0..1), [(0..0, Id(0))]);
    assert_iter_eq(tree.range(0..2), [(0..0, Id(0)), (1..1, Id(1))]);
    assert_iter_eq(tree.range(1..2), [(1..1, Id(1))]);
    assert_iter_eq(tree.range(2..2), []);

    (2..100).for_each(|i| drop(tree.insert(i, Id(i))));

    assert_iter_eq(tree.range(0..0), []);
    assert_iter_eq(tree.range(0..1), [(0..0, Id(0))]);
    assert_iter_eq(tree.range(0..2), [(0..0, Id(0)), (1..1, Id(1))]);
    assert_iter_eq(tree.range(1..2), [(1..1, Id(1))]);
    assert_iter_eq(tree.range(2..2), []);

    assert_iter_eq(tree.range(0..3), [(0..0, Id(0)), (1..1, Id(1)), (2..2, Id(2))]);
    assert_iter_eq(tree.range(0..=2), [(0..0, Id(0)), (1..1, Id(1)), (2..2, Id(2))]);
    assert_iter_eq(tree.range(20..40), (20..40).map(|i| (i..i, Id(i))));
    assert_iter_eq(tree.range(80..100), (80..100).map(|i| (i..i, Id(i))));
    assert_iter_eq(tree.range(80..=100), (80..100).map(|i| (i..i, Id(i))));
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
    tree.insert(0, Id(1));

    assert_iter_eq(tree.range(..), [(0..0, Id(0)), (0..0, Id(1))]);

    tree.delete(0);
    assert_iter_eq(tree.range(..), [(0..0, Id(1))]);

    tree.delete(1);
    assert_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_bulk_delete() {
    let mut tree = new(10000);
    let k = 2000;
    (0..k).for_each(|i| drop(tree.insert(i, Id(i))));
    (0..k).for_each(|i| {
        assert_eq!(tree.delete(Id(i)), Some(i..i));
        assert_iter_eq(tree.range(..), (i + 1..k).map(|j| (j..j, Id(j))));
    });
}

#[test]
fn marktree_drain_2() {
    let mut tree = new(10);
    (0..4).for_each(|i| drop(tree.insert(i, Id(i))));

    assert_iter_eq(tree.drain(0..=0), [(0..0, Id(0))]);
    assert_iter_eq(tree.range(..), [(1..1, Id(1)), (2..2, Id(2)), (3..3, Id(3))]);

    assert_iter_eq(tree.drain(1..=1), [(1..1, Id(1))]);
    assert_iter_eq(tree.range(..), [(2..2, Id(2)), (3..3, Id(3))]);

    tree.drain(2..=2);
    assert_iter_eq(tree.range(..), [(3..3, Id(3))]);

    tree.drain(3..=3);
    assert_iter_eq(tree.range(..), []);

    tree.insert(0, Id(0));
    tree.insert(0, Id(1));
    assert_iter_eq(tree.range(..), [(0..0, Id(0)), (0..0, Id(1))]);

    tree.drain(0..0);
    assert_iter_eq(tree.range(..), [(0..0, Id(0)), (0..0, Id(1))]);

    tree.drain(0..1);
    assert_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_drain_1() {
    let mut tree = new(10);

    tree.insert(0, Id(0));
    assert_iter_eq(tree.range(..), [(0..0, Id(0))]);
    assert_eq!(tree.len(), 10);

    assert_iter_eq(tree.drain(0..1), [(0..0, Id(0))]);
    assert_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 10);

    tree.insert(1, Id(1));
    assert_iter_eq(tree.range(..), [(1..1, Id(1))]);
    assert_eq!(tree.len(), 10);

    assert_iter_eq(tree.drain(0..1), []);
    assert_iter_eq(tree.range(..), [(1..1, Id(1))]);
    assert_eq!(tree.len(), 10);

    assert_iter_eq(tree.drain(0..2), [(1..1, Id(1))]);
    assert_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 10);
}

#[test]
fn marktree_bulk_drain() {
    const LEN: usize = 200;
    let mut tree = new(LEN);

    (0..100).for_each(|i| drop(tree.insert(i, Id(i))));
    assert_iter_eq(tree.range(..), (0..100).map(|i| (i..i, Id(i))));
    assert_eq!(tree.len(), LEN);

    tree.drain(0..20);
    assert_iter_eq(tree.range(..), (20..100).map(|i| (i..i, Id(i))));
    assert_eq!(tree.len(), LEN);

    tree.drain(80..100);
    assert_iter_eq(tree.range(..), (20..80).map(|i| (i..i, Id(i))));
    assert_eq!(tree.len(), LEN);
}

#[test]
fn marktree_simple_insert() {
    let mut tree = new(2);
    tree.insert(1, Id(0));

    assert_iter_eq(tree.range(..), [(1..1, Id(0))]);

    tree.insert(1, Id(1));
    assert_iter_eq(tree.range(..), [(1..1, Id(0)), (1..1, Id(1))]);

    tree.insert(0, Id(2));
    assert_iter_eq(tree.range(..), [(0..0, Id(2)), (1..1, Id(0)), (1..1, Id(1))]);
}

#[test]
fn marktree_small_insert() {
    let mut tree = new(5);
    (0..5).for_each(|i| {
        tree.insert(i, Id(i));
        assert_iter_eq(tree.range(..), (0..=i).map(|j| (j..j, Id(j))));
    });
}

#[test]
fn marktree_split() {
    let mut tree = new(100);
    (0..100).for_each(|i| drop(tree.insert(i, Id(i))));
    assert_iter_eq(tree.range(..), (0..100).map(|i| (i..i, Id(i))));
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
            assert_iter_eq(tree.range(..).map(|(a, _)| a), (0..=at).map(|i| i..i));
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
    assert_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 2);
}

#[test]
fn marktree_shift_append_delete() {
    let mut tree = new(1);

    tree.shift(0..0, 1);
    assert_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 2);

    tree.shift(0..1, 0);
    assert_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 1);
}

// TODO test lots of shift deletions

#[test]
fn marktree_shift_delete() {
    let mut tree = new(3);
    assert_eq!(tree.len(), 3);
    tree.shift(0..3, 1);

    assert_iter_eq(tree.range(..), []);
    assert_eq!(tree.len(), 1);
}

#[test]
fn marktree_shift_empty() {
    let mut tree = new(5);
    assert_eq!(tree.len(), 5);

    tree.shift(0..1, 0);
    assert_eq!(tree.len(), 4);

    tree.shift(1..1, 1);
    assert_eq!(tree.len(), 5);
}

#[test]
fn marktree_shift_simple() {
    let mut tree = new(10);

    tree.insert(1, Id(0));
    assert_iter_eq(tree.range(..), [(1..1, Id(0))]);

    tree.shift(0..0, 2);

    assert_iter_eq(tree.range(..), [(3..3, Id(0))]);
    assert_eq!(tree.len(), 12);

    tree.shift(0..1, 0);
    assert_eq!(tree.len(), 11);
    assert_iter_eq(tree.range(..), [(2..2, Id(0))]);
}

#[test]
fn marktree_shift() {
    let mut tree = new(10);
    tree.insert(0, Id(0));
    tree.insert(1, Id(1));
    tree.shift(0..0, 2);
    assert_iter_eq(tree.range(..), [(2..2, Id(0)), (3..3, Id(1))]);
}

#[test]
fn marktree_smoke() {
    let mut tree = new(10);
    assert_eq!(tree.len(), 10);
    tree.insert(0, Id(0));
    tree.insert(3, Id(1));

    assert_iter_eq(tree.range(..), [(0..0, Id(0)), (3..3, Id(1))]);

    tree.insert(3, Id(2));
    assert_iter_eq(tree.range(..), [(0..0, Id(0)), (3..3, Id(1)), (3..3, Id(2))]);

    tree.insert(2, Id(4));
    assert_iter_eq(tree.range(..), [(0..0, Id(0)), (2..2, Id(4)), (3..3, Id(1)), (3..3, Id(2))]);
    assert_eq!(tree.len(), 10);
}

#[test]
fn marktree_get() {
    let mut tree = new(10);
    assert_eq!(tree.len(), 10);

    tree.insert(0, Id(0));
    assert_eq!(tree.get(0), Some(0..0));

    tree.insert(3, Id(1));
    assert_eq!(tree.get(1), Some(3..3));

    tree.insert(3, Id(2));
    assert_eq!(tree.get(2), Some(3..3));

    tree.insert(2, Id(4));
    assert_eq!(tree.get(4), Some(2..2));

    assert_eq!(tree.get(0), Some(0..0));
    assert_eq!(tree.get(1), Some(3..3));
    assert_eq!(tree.get(2), Some(3..3));
    assert_eq!(tree.get(3), None);
    assert_eq!(tree.get(4), Some(2..2));
}

#[test]
fn marktree_duplicate_offsets() {
    let mut tree = new(10);
    (0..1000).for_each(|i| drop(tree.insert(0, Id(i))));
    assert_iter_eq(tree.range(..), (0..1000).map(|i| (0..0, Id(i))));
    assert_iter_eq(tree.drain(0..=0), (0..1000).map(|i| (0..0, Id(i))));
    assert_iter_eq(tree.range(..), []);
}

#[test]
fn marktree_bulk_get() {
    let mut tree = new(10000);
    let k = 4000;
    (0..k).for_each(|i| drop(tree.insert(i, Id(i))));
    (0..k).for_each(|i| {
        let offset = tree.get(Id(i));
        assert_eq!(offset, Some(i..i));
    });
}

#[test]
fn marktree_left_bias() {
    let mut tree = new(1);
    tree.insert(0, Id(0)).start_bias(Bias::Left);
    tree.shift(0..0, 1);
    // Should not shift due to bias.
    assert_iter_eq(tree.range(..), [(0..0, Id(0))]);
}

#[test]
fn marktree_bias() {
    let mut tree = new(5);
    tree.insert(0, Id(0)).start_bias(Bias::Left);
    tree.insert(0, Id(1)).start_bias(Bias::Right);
    tree.shift(0..0, 1);
    assert_iter_eq(tree.range(..), [(0..0, Id(0)), (1..1, Id(1))]);
}

#[test]
fn marktree_range_mark() {
    let mut tree = new(5);
    tree.insert(0, Id(0)).width(1);
    assert_iter_eq(tree.range(..), [(0..1, Id(0))]);

    assert_eq!(tree.delete(Id(0)), Some(0..1));
    assert_iter_eq(tree.range(..), []);

    tree.insert(0, Id(1)).width(2);
    assert_iter_eq(tree.range(..), [(0..2, Id(1))]);

    tree.insert(1, Id(2)).width(3);
    assert_iter_eq(tree.range(..), [(0..2, Id(1)), (1..4, Id(2))]);

    tree.shift(0..0, 1);
    assert_iter_eq(tree.range(..), [(1..3, Id(1)), (2..5, Id(2))]);
}

#[test]
fn marktree_shift_range_mark() {
    #[track_caller]
    fn check(
        at: usize,
        f: impl FnOnce(Inserter<'_, Id, 4>) -> Inserter<'_, Id, 4>,
        (shift_range, by): (Range<usize>, usize),
        expected: (Range<usize>, Id),
    ) {
        let mut tree = new(5);
        drop(f(tree.insert(at, Id(0))));
        tree.shift(shift_range, by);
        assert_iter_eq(tree.range(..), iter::once(expected));
    }

    check(0, |i| i.width(2), (1..1, 1), (0..3, Id(0)));
}

#[test]
fn marktree_regression_1() {
    let mut tree = new(10);
    tree.insert(0, Id(0)).width(1);
    tree.insert(5, Id(1)).width(1);
    tree.insert(1, Id(2)).width(1);

    assert_iter_eq(tree.range(..), [(0..1, Id(0)), (1..2, Id(2)), (5..6, Id(1))]);
    tree.insert(5, Id(3)).width(1);
    assert_eq!(tree.len(), 10);
}

#[test]
fn repro2() {
    let n = 1000;
    let mut tree = new(n);
    let at = [0, 0, 1, 907, 0, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 875, 0];
    let widths = [66, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 32];
    for (i, &at) in at.iter().enumerate() {
        let width = if widths.is_empty() { 0 } else { widths[i % widths.len()] };

        tree.insert(at, Id(i)).width(width);
        assert_eq!(tree.get(Id(i)), Some(at..at + width));
        assert_eq!(tree.len(), n);
    }
}

proptest::proptest! {
    #[test]
    fn marktree_prop(at in vec(0..1000usize, 0..100), widths in vec(1..100usize, 0..100)) {
        let n = 10000;
        let mut tree = new(n);
        for (i, &at) in at.iter().enumerate() {
            let width = if widths.is_empty() { 0 } else { widths[i % widths.len()] };

            tree.insert(at, Id(i)).width(width);
            assert_eq!(tree.get(Id(i)), Some(at..at + width));
            assert_eq!(tree.len(), n);
        }
    }
}
