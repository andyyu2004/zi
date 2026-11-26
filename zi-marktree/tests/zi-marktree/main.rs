#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::{fmt, iter};

use proptest::collection::vec;
use proptest::strategy::{BoxedStrategy, Just, Strategy};
use proptest::{prop_compose, prop_oneof};
use zi_marktree::{Bias, Inserter, MarkBuilder, MarkTree, MarkTreeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
fn marktree_get_empty_range_mark() {
    let mut tree = new(1000);
    for i in 0..100 {
        tree.insert(i * 5, Id(i)).width(2);
    }
    for i in (0..100).rev() {
        tree.shift(i * 5..i * 5 + 2, 0);
    }
    for i in 0..100 {
        assert_eq!(tree.get(Id(i)), Some(i * 3..i * 3));
    }
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
#[should_panic(expected = "does not fit in 61 bits")]
fn marktree_id_too_large() {
    let mut tree = new(10);
    tree.insert(0, Id(1 << 61));
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
fn marktree_delete_range_marks() {
    let mut tree = new(2000);
    let mut expected = BTreeMap::new();
    for i in 0..200 {
        let width = [0, 1, 7, 300][i % 4];
        tree.insert(i * 5, Id(i)).width(width);
        expected.insert(Id(i), i * 5..i * 5 + width);
    }

    for i in (0..200).rev().step_by(3).chain((0..200).step_by(3)) {
        assert_eq!(tree.delete(Id(i)), expected.remove(&Id(i)));
        assert_eq!(tree.delete(Id(i)), None);
        assert_eq!(tree.len(), 2000);
        for (&id, range) in &expected {
            assert_eq!(tree.get(id), Some(range.clone()));
        }
        let mut actual = tree.range(..).collect::<Vec<_>>();
        actual.sort_by_key(|&(_, id)| id);
        assert_iter_eq(actual, expected.iter().map(|(&id, range)| (range.clone(), id)));
    }
}

#[test]
fn marktree_delete_zero_width_range_mark() {
    let mut tree = new(1000);
    let ids = (0..71).chain(90..100);
    ids.clone().for_each(|i| drop(tree.insert(i * 10, Id(i))));
    tree.insert(500, Id(100)).width(3);
    tree.insert(700, Id(101)).width(200);
    tree.shift(500..503, 0);
    tree.shift(697..897, 0);

    assert_eq!(tree.delete(Id(100)), Some(500..500));
    assert_eq!(tree.delete(Id(101)), Some(697..697));
    assert_eq!(tree.get(Id(50)), Some(500..500));
    assert_eq!(tree.get(Id(51)), Some(507..507));
    assert_eq!(tree.get(Id(70)), Some(697..697));
    assert_eq!(tree.get(Id(90)), Some(697..697));
    assert_eq!(tree.get(Id(91)), Some(707..707));
    let mut actual = tree.range(..).map(|(_, id)| id).collect::<Vec<_>>();
    actual.sort();
    assert_iter_eq(actual, ids.map(Id));
}

#[test]
fn marktree_shift_delete_whole_leaves() {
    let mut tree = new(1000);
    (0..100).for_each(|i| drop(tree.insert(i * 10, Id(i))));
    tree.shift(697..897, 0);

    for i in 0..100 {
        let at = match i * 10 {
            at if at < 697 => at,
            at if at <= 897 => 697,
            at => at - 200,
        };
        assert_eq!(tree.get(Id(i)), Some(at..at));
    }
    assert_eq!(tree.range(..).count(), 100);
    assert_eq!(tree.delete(Id(80)), Some(697..697));
}

#[test]
fn marktree_shift_delete_whole_leaves_point_mark() {
    let mut tree = new(1000);
    tree.insert(415, Id(0)).width(6);
    tree.shift(0..2, 0);
    tree.insert(406, Id(0));
    tree.insert(407, Id(0));
    tree.insert(1, Id(1)).width(1);
    tree.shift(400..414, 0);

    assert_eq!(tree.get(Id(0)), Some(400..400));
    assert_iter_eq(tree.range(..), [(1..2, Id(1)), (400..400, Id(0))]);
}

#[test]
fn marktree_shift_delete_whole_leaves_range_mark() {
    let mut tree = new(1000);
    (0..4).for_each(|_| drop(tree.insert(0, Id(0))));
    tree.insert(1, Id(0)).width(1);
    tree.insert(341, Id(1)).width(1);
    tree.insert(1, Id(2)).width(2);
    tree.shift(260..343, 0);

    assert_iter_eq(tree.range(..), [(1..2, Id(0)), (1..3, Id(2)), (260..260, Id(1))]);
    assert_eq!(tree.delete(Id(1)), Some(260..260));
}

#[test]
fn marktree_shift_delete_whole_last_leaf() {
    let mut tree = new(1000);
    tree.insert(696, Id(0)).width(13);
    tree.shift(617..696, 3);
    tree.insert(535, Id(2)).width(1);
    tree.insert(1, Id(0)).width(1);
    // The leaves are now 0..535, 535..620 and 620..924, the second holding only mark 2.
    tree.shift(535..620, 0);

    assert_eq!(tree.len(), 839);
    assert_iter_eq(tree.range(..), [(1..2, Id(0)), (535..535, Id(2))]);
}

#[test]
fn marktree_shift_replace_whole_leaves() {
    let mut tree = new(1000);
    (0..100).for_each(|i| drop(tree.insert(i * 10, Id(i))));
    tree.insert(300, Id(100)).start_bias(Bias::Left);
    tree.insert(290, Id(101)).width(10).end_bias(Bias::Left);
    tree.shift(300..500, 7);

    assert_eq!(tree.get(Id(29)), Some(290..290));
    assert_eq!(tree.get(Id(30)), Some(307..307));
    assert_eq!(tree.get(Id(50)), Some(307..307));
    assert_eq!(tree.get(Id(51)), Some(317..317));
    assert_eq!(tree.get(Id(100)), Some(300..300));
    assert_eq!(tree.get(Id(101)), Some(290..300));
    assert_eq!(tree.range(..).count(), 102);
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
fn marktree_range_includes_eof() {
    #[track_caller]
    fn check(mut tree: MarkTree<Id, 4>) {
        assert_eq!(tree.get(Id(0)), Some(5..5));
        assert_iter_eq(tree.range(..), [(2..5, Id(1)), (5..5, Id(0))]);
        assert_iter_eq(tree.range(5..), [(5..5, Id(0))]);
        assert_iter_eq(tree.drain(3..), [(5..5, Id(0))]);
        assert_iter_eq(tree.drain(..), [(2..5, Id(1))]);
        assert_iter_eq(tree.range(..), []);
    }

    let mut tree = new(5);
    tree.insert(5, Id(0));
    tree.insert(2, Id(1)).width(3);
    check(tree);
    check(MarkTree::build(
        5,
        [(Id(0), MarkBuilder::new(5)), (Id(1), MarkBuilder::new(2).width(3))],
    ));
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
fn marktree_drain_range_marks() {
    let mut tree = new(20);
    tree.insert(2, Id(0));
    tree.insert(3, Id(1)).width(2);
    tree.insert(4, Id(2)).width(10);
    tree.insert(1, Id(3)).width(5);
    tree.insert(8, Id(4)).width(1);
    tree.insert(5, Id(5));

    assert_iter_eq(tree.drain(5..5), []);
    assert_iter_eq(tree.drain(2..6), [(2..2, Id(0)), (3..5, Id(1)), (4..14, Id(2)), (5..5, Id(5))]);
    assert_eq!(tree.len(), 20);
    for id in [0, 1, 2, 5] {
        assert_eq!(tree.get(Id(id)), None);
    }
    assert_iter_eq(tree.range(..), [(1..6, Id(3)), (8..9, Id(4))]);

    let mut drain = tree.drain(..);
    assert_eq!(drain.len(), 2);
    assert_eq!(drain.next(), Some((1..6, Id(3))));
    assert_eq!(drain.len(), 1);
    drop(drain);
    assert_iter_eq(tree.range(..), []);
    assert_eq!(tree.get(Id(4)), None);
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
        for &(at, i) in &values {
            tree.insert(at, i);
            assert_eq!(tree.len(), LEN);
            assert_iter_eq(tree.range(..).map(|(a, _)| a), (0..=at).map(|i| i..i));
        }
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
fn marktree_range_window() {
    fn check<const N: usize>() {
        let mut tree = MarkTree::<Id, N>::new(100);
        tree.insert(5, Id(0)).width(10);
        tree.insert(20, Id(1)).width(2);
        tree.insert(21, Id(2));
        tree.insert(22, Id(3)).width(30);
        tree.insert(40, Id(4));
        tree.insert(45, Id(5)).width(5);
        tree.insert(46, Id(6));

        assert_iter_eq(
            tree.range(..),
            [
                (5..15, Id(0)),
                (20..22, Id(1)),
                (21..21, Id(2)),
                (22..52, Id(3)),
                (40..40, Id(4)),
                (45..50, Id(5)),
                (46..46, Id(6)),
            ],
        );
        assert_iter_eq(tree.range(10..30), [(20..22, Id(1)), (21..21, Id(2)), (22..52, Id(3))]);
        assert_iter_eq(tree.range(20..21), [(20..22, Id(1))]);
        assert_iter_eq(tree.range(41..47), [(45..50, Id(5)), (46..46, Id(6))]);

        tree.shift(45..50, 0);
        let mut collapsed = tree.range(45..46).collect::<Vec<_>>();
        collapsed.sort_by_key(|&(_, id)| id);
        assert_iter_eq(collapsed, [(45..45, Id(5)), (45..45, Id(6))]);
        tree.shift(5..15, 0);
        assert_iter_eq(tree.range(5..6), [(5..5, Id(0))]);
        assert_iter_eq(tree.range(..6), [(5..5, Id(0))]);
    }

    check::<4>();
    check::<32>();
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
        let mut len = 10000;
        let mut tree = new(len);
        let mut insertions = BTreeMap::new();
        for action in at {
            match action {
                Action::Insert { at, width, id, start_bias, end_bias } if at + width <= len => {
                    insertions.insert(id, Mark { range: at..at + width, start_bias, end_bias });
                    tree.insert(at, id).width(width).start_bias(start_bias).end_bias(end_bias);
                }
                Action::Delete(id) => {
                    assert_eq!(tree.delete(id), insertions.remove(&id).map(|mark| mark.range));
                }
                Action::Drain(range) => {
                    let mut drained = tree.drain(range.clone()).collect::<Vec<_>>();
                    drained.sort_by_key(|&(_, id)| id);
                    let expected = insertions
                        .extract_if(.., |_, mark| range.contains(&mark.range.start))
                        .map(|(id, mark)| (mark.range, id))
                        .collect::<Vec<_>>();
                    assert_eq!(drained, expected);
                    for (_, id) in drained {
                        assert_eq!(tree.get(id), None);
                    }
                }
                // Not up to the end: `range(..)` skips marks at the end (zi-editor/zi-tracking#12).
                Action::Shift { range, by } if range.end < len => {
                    tree.shift(range.clone(), by);
                    len = len - range.len() + by;
                    for mark in insertions.values_mut() {
                        let Range { start, end } = mark.range;
                        mark.range = shift_offset(start, mark.start_bias, &range, by)
                            ..shift_offset(end, mark.end_bias, &range, by);
                    }
                }
                // Out of bounds of the shrunk tree.
                Action::Insert { .. } | Action::Shift { .. } => {}
            }

            assert_eq!(tree.len(), len);

            // `get` can panic on a range mark collapsed to zero width (zi-editor/zi-tracking#12).
            for (&id, mark) in insertions.iter().filter(|(_, mark)| !mark.range.is_empty()) {
                assert_eq!(tree.get(id), Some(mark.range.clone()));
            }
            let mut actual = tree.range(..).collect::<Vec<_>>();
            actual.sort_by_key(|&(_, id)| id);
            assert_iter_eq(actual, insertions.iter().map(|(&id, mark)| (mark.range.clone(), id)));
        }
    }

    #[test]
    fn marktree_prop_range(
        actions in vec(arb_range_action(), 0..100),
        windows in vec((0..1100usize, 0..200usize), 1..10),
    ) {
        check_range::<4>(&actions, &windows);
        check_range::<32>(&actions, &windows);
    }
}

#[derive(Debug, Clone)]
enum RangeAction {
    Insert { at: usize, width: usize, id: Id },
    Delete(Id),
    Shift { range: Range<usize>, by: usize },
}

fn arb_range_action() -> BoxedStrategy<RangeAction> {
    prop_oneof![
        3 => (0..1000usize, 0..20usize, 0..100usize)
            .prop_map(|(at, width, id)| RangeAction::Insert { at, width, id: Id(id) }),
        1 => (0..100usize).prop_map(Id).prop_map(RangeAction::Delete),
        1 => (0..1000usize, 0..100usize, 0..5usize)
            .prop_map(|(at, len, by)| RangeAction::Shift { range: at..at + len, by }),
    ]
    .boxed()
}

fn apply_range_actions<const N: usize>(actions: &[RangeAction]) -> (MarkTree<Id, N>, BTreeSet<Id>) {
    let mut tree = MarkTree::<Id, N>::new(1000);
    let mut ids = BTreeSet::new();
    for action in actions {
        match *action {
            RangeAction::Insert { at, width, id } => {
                if at + width <= tree.len() {
                    ids.insert(id);
                    tree.insert(at, id).width(width);
                }
            }
            RangeAction::Delete(id) => {
                ids.remove(&id);
                tree.delete(id);
            }
            RangeAction::Shift { ref range, by } => {
                if range.end < tree.len() {
                    tree.shift(range.clone(), by);
                }
            }
        }
    }
    (tree, ids)
}

fn check_range<const N: usize>(actions: &[RangeAction], windows: &[(usize, usize)]) {
    let (tree, ids) = apply_range_actions::<N>(actions);
    let ranges = ids.iter().map(|&id| (tree.get(id).unwrap(), id)).collect::<Vec<_>>();

    for &(start, len) in windows {
        let window = start..start + len;
        let mut expected = ranges
            .iter()
            .filter(|(range, _)| window.contains(&range.start))
            .cloned()
            .collect::<Vec<_>>();
        let mut actual = tree.range(window).collect::<Vec<_>>();
        assert!(actual.is_sorted_by_key(|(range, _)| range.start));
        expected.sort_by_key(|&(ref range, id)| (range.start, id));
        actual.sort_by_key(|&(ref range, id)| (range.start, id));
        assert_eq!(actual, expected);
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
    Insert { at: usize, width: usize, id: Id, start_bias: Bias, end_bias: Bias },
    Delete(Id),
    Drain(Range<usize>),
    Shift { range: Range<usize>, by: usize },
}

#[derive(Debug)]
struct Mark {
    range: Range<usize>,
    start_bias: Bias,
    end_bias: Bias,
}

/// Where a key at `at` with the given bias is after `shift(range, by)`.
fn shift_offset(at: usize, bias: Bias, range: &Range<usize>, by: usize) -> usize {
    if at < range.start || (at == range.start && bias == Bias::Left) {
        at
    } else if at <= range.end {
        range.start + by
    } else {
        at - range.len() + by
    }
}

/// `(start_bias, end_bias)` pairs. A right-biased start with a left-biased end can put the end
/// before the start (zi-editor/zi-tracking#34).
fn arb_biases() -> impl Strategy<Value = (Bias, Bias)> {
    prop_oneof![
        Just((Bias::Left, Bias::Left)),
        Just((Bias::Left, Bias::Right)),
        Just((Bias::Right, Bias::Right)),
    ]
}

prop_compose! {
    fn arb_action_insert()(
        at in 0..1000usize,
        width in 0..100usize,
        id in 0..100usize,
        (start_bias, end_bias) in arb_biases(),
    ) -> Action {
        Action::Insert { at, width, id: Id(id), start_bias, end_bias }
    }
}

fn arb_action() -> BoxedStrategy<Action> {
    prop_oneof![
        arb_action_insert(),
        (0..1000usize).prop_map(Id).prop_map(Action::Delete),
        (0..1100usize, 0..200usize).prop_map(|(start, len)| Action::Drain(start..start + len)),
        // Only deleting shifts: an insertion at a leaf boundary can move left-biased marks
        // (zi-editor/zi-tracking#12).
        (0..1100usize, 1..200usize, 0..20usize)
            .prop_map(|(start, len, by)| Action::Shift { range: start..start + len, by }),
    ]
    .boxed()
}

#[test]
fn marktree_create_delete_stays_small() {
    let n = 1000;
    let mut expected = new(n);
    expected.insert(500, Id(0));
    let mut tree = new(n);
    tree.insert(500, Id(0));

    for i in 1..2000 {
        let at = 1 + (i * 37) % (n - 10);
        let width = i % 5;
        tree.insert(at, Id(i)).width(width);
        assert_eq!(tree.delete(Id(i)), Some(at..at + width));
    }

    assert_eq!(format!("{tree:?}"), format!("{expected:?}"));
    assert_iter_eq(tree.range(..), [(500..500, Id(0))]);
    assert_eq!(tree.get(Id(0)), Some(500..500));
}
