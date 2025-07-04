#![feature(anonymous_lifetime_in_impl_trait)]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::collections::BTreeMap;
use std::ops::Range;
use std::{fmt, iter};

use proptest::collection::vec;
use proptest::strategy::{BoxedStrategy, Strategy};
use proptest::{prop_compose, prop_oneof};
use zi_marktree::{Bias, Inserter, MarkBuilder, MarkTree, MarkTreeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Id(usize);

impl From<Id> for u32 {
    #[inline]
    fn from(id: Id) -> u32 {
        debug_assert!(id.0 < u32::MAX as usize, "id.0 = {}", id.0);
        id.0 as u32
    }
}

impl From<u32> for Id {
    #[inline]
    fn from(id: u32) -> Id {
        Id(id as usize)
    }
}

impl MarkTreeId for Id {}

#[track_caller]
fn assert_iter_eq<T: Eq + fmt::Debug + 'static>(
    values: impl IntoIterator<Item = T>,
    expected: impl IntoIterator<Item = T>,
) {
    let values = values.into_iter().collect::<Vec<_>>();
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
fn marktree_shift_end_equal_start() {
    let mut tree = new(5);
    tree.insert(0, Id(0)).width(2);
    assert_iter_eq(tree.range(..), [(0..2, Id(0))]);
    tree.shift(0..2, 0);
    assert_eq!(tree.len(), 3);
    assert_iter_eq(tree.range(..), [(0..0, Id(0))]);
    assert_eq!(tree.delete(Id(0)), Some(0..0));
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
    let k = 1000;
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
fn marktree_get_simple() {
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
    let k = 2000;
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
fn marktree_inserts() {
    check_inserts(100, [1, 2], [2, 2]);
    check_inserts(100, [1], []);
    check_inserts(100, [0], [1]);
    check_inserts(10000, [0, 923, 67, 923], [1]);
}

#[test]
fn marktree_build() {
    check_build(100, [0], [1]);
    check_build(100, [1], [1]);
    check_build(
        1000,
        [
            41, 1, 31, 74, 28, 34, 18, 78, 55, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0,
        ],
        [1, 1],
    );
}

proptest::proptest! {
    #[test]
    fn marktree_prop_insert(at in vec(0..1000usize, 0..100), widths in vec(1..100usize, 0..100)) {
        check_inserts(10000, at, widths);
    }

    #[test]
    fn marktree_prop_build(at in vec(0..1000usize, 0..100), widths in vec(1..100usize, 1..100)) {
        check_build(10000, at, widths);
    }

    #[test]
    fn marktree_prop_insert_delete(at in vec(arb_action(), 0..100)) {
        let n = 10000;
        let mut tree = new(n);
        let mut insertions = BTreeMap::new();
        for action in at {
            match action {
                Action::Insert { at, width, id } => {
                    insertions.insert(id, at..at + width) ;
                    tree.insert(at, id).width(width);
                }
                Action::Delete(id) => {
                    assert_eq!(tree.delete(id), insertions.remove(&id))
                }
            }

            assert_eq!(tree.len(), n);

            for (&id, range) in &insertions {
                assert_eq!(tree.get(id), Some(range.clone()));
            }
        }
    }
}

fn check_inserts(
    n: usize,
    at: impl IntoIterator<Item = usize>,
    widths: impl IntoIterator<Item = usize>,
) -> MarkTree<Id, 4> {
    let mut tree = new(n);
    let mut insertions = BTreeMap::new();
    let widths = widths.into_iter().collect::<Vec<_>>();
    for (i, at) in at.into_iter().enumerate() {
        let width = if widths.is_empty() { 0 } else { widths[i % widths.len()] };

        assert!(insertions.insert(Id(i), at..at + width).is_none());
        tree.insert(at, Id(i)).width(width);

        assert_eq!(tree.len(), n, "mark insertion should never change length of tree");

        for (&id, range) in &insertions {
            assert_eq!(tree.get(id), Some(range.clone()));
        }
    }

    tree
}

fn check_build(
    n: usize,
    at: impl IntoIterator<Item = usize>,
    widths: impl IntoIterator<Item = usize>,
) -> MarkTree<Id, 4> {
    let at = at.into_iter().collect::<Vec<_>>();
    let widths = widths.into_iter().collect::<Vec<_>>();
    let insertions = at
        .iter()
        .zip(widths.iter().cycle())
        .enumerate()
        .map(|(id, (&at, &width))| (Id(id), MarkBuilder::new(at).width(width)));

    let tree = MarkTree::<Id, 4>::build(n, insertions);

    // builder should produce the same tree as manual insertions
    let mut actual = tree.range(..).collect::<Vec<_>>();
    assert!(actual.is_sorted_by_key(|(range, _)| range.start));

    let mut expected = check_inserts(n, at, widths).range(..).collect::<Vec<_>>();
    assert!(expected.is_sorted_by_key(|(range, _)| range.start));

    // We need to explicitly sort because we don't guarantee the order of the end points
    // if the start points are equal.
    // Similarly, if both start and end points are equal, we don't guarantee ordering about ids.

    actual.sort_by(|(a, x), (b, y)| {
        a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)).then_with(|| x.cmp(y))
    });
    expected.sort_by(|(a, x), (b, y)| {
        a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)).then_with(|| x.cmp(y))
    });

    assert_iter_eq(actual, expected);
    tree
}

#[derive(Debug)]
enum Action {
    Insert { at: usize, width: usize, id: Id },
    Delete(Id),
}

prop_compose! {
    fn arb_action_insert()(
        at in 0..1000usize,
        width in 1..100usize,
        id in 0..100usize,
    ) -> Action {
        Action::Insert { at, width, id: Id(id) }
    }
}

fn arb_action() -> BoxedStrategy<Action> {
    prop_oneof![arb_action_insert(), (0..1000usize).prop_map(Id).prop_map(Action::Delete)].boxed()
}
