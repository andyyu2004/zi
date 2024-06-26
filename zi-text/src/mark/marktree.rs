use std::collections::VecDeque;
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::ops::{Add, AddAssign, RangeBounds, Sub, SubAssign};
use std::{fmt, iter};

use arrayvec::ArrayVec;
use crop::tree::{
    Arc, AsSlice, BalancedLeaf, BaseMeasured, Lnode, Metric, Node, ReplaceableLeaf, Summarize, Tree,
};
use roaring::RoaringTreemap;
use stdx::bound::BoundExt;
use stdx::iter::ExactChain;

use crate::Deltas;

pub trait MTreeId: Copy + Eq + From<u64> + Into<u64> + fmt::Debug + 'static {}

impl<Id: Copy + Eq + fmt::Debug + From<u64> + Into<u64> + 'static> MTreeId for Id {}

pub trait MTree<Id: MTreeId> {
    fn new(n: usize) -> Self;

    fn len(&self) -> usize;

    fn get(&self, id: Id) -> Option<usize>;

    fn range(&self, range: impl RangeBounds<usize>) -> impl Iterator<Item = (usize, Id)>;

    fn insert(&mut self, at: usize, id: Id);

    fn delete(&mut self, id: Id) -> Option<usize>;

    fn shift(&mut self, range: impl RangeBounds<usize>, by: usize);

    fn drain(&mut self, range: impl RangeBounds<usize>) -> Drain<'_, Id, Self>
    where
        Self: Sized,
    {
        let ids = self.range(range).map(|(_, id)| id).collect::<Vec<_>>().into_iter();
        Drain { tree: self, ids }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Bias {
    // Important that `Self::Left < Self::Right`
    #[default]
    Left,
    Right,
}

// Small to make it easier to debug tests.
#[cfg(debug_assertions)]
const ARITY: usize = 4;

// FIXME: currently completely arbitrary number
#[cfg(not(debug_assertions))]
const ARITY: usize = 7;

/// A tree of ordered items that each have a byte position.
/// This can be edited efficiently (logarithmic time) with `Deltas`.
//
// This is implemented in a way that is basically a rope but instead of actually representing
// string data it's compressed into `Gap`s with `Item`s in between.
// The `Item` are always zero-width and are used to represent the byte positions of the items.
//
// Plenty of optimizations available. The implementation is fairly naive.
//  - avoid recreating bitmaps and arrays from scratch all the time
#[derive(Debug)]
pub struct MarkTree<Id: MTreeId, const N: usize> {
    tree: Tree<ARITY, Leaf<N>>,
    _id: PhantomData<Id>,
}

impl<const N: usize, Id: MTreeId> MTree<Id> for MarkTree<Id, N> {
    fn new(n: usize) -> Self {
        Self::new(n)
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn get(&self, id: Id) -> Option<usize> {
        self.get(id)
    }

    fn range(&self, range: impl RangeBounds<usize>) -> impl Iterator<Item = (usize, Id)> {
        self.items(range)
    }

    fn insert(&mut self, at: usize, id: Id) {
        self.insert(at, id)
    }

    fn delete(&mut self, id: Id) -> Option<usize> {
        self.delete(id)
    }

    fn shift(&mut self, range: impl RangeBounds<usize>, by: usize) {
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        self.tree.replace(ByteMetric(start)..ByteMetric(end), Replacement::Shift(by));
    }
}

impl<const N: usize, Id: MTreeId> MarkTree<Id, N> {
    /// Creates a new `MarkTree` with a single gap of `n` bytes.
    /// This should be equal to the length of the text in bytes.
    pub fn new(n: usize) -> Self {
        let mut this = Self { tree: Tree::default(), _id: PhantomData };
        if let Some(n) = NonZeroUsize::new(n) {
            this.replace(.., Replacement::Gap(n));
        }
        this
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tree.summary().bytes
    }

    pub fn get(&self, id: Id) -> Option<usize> {
        let id = id.into();
        let (leaf_offset, leaf) = self.find(id)?;
        let slice = leaf.as_slice();
        let offset = slice.get(id)?;
        Some(leaf_offset + offset)
    }

    fn find(&self, id: u64) -> Option<(usize, &Lnode<Leaf<N>>)> {
        // Need to do a manual traversal to make use of the bitmaps.
        let mut node = self.tree.root().as_ref();
        if !node.summary().ids.contains(id) {
            return None;
        }
        return None;

        let mut offset = 0;

        loop {
            debug_assert!(node.summary().ids.contains(id));
            todo!();
            // match node {
            //     Node::Internal(inode) => {
            //         node = inode
            //             .children()
            //             .iter()
            //             .find(|child| {
            //                 let summary = child.summary();
            //                 if summary.ids.contains(raw_id) {
            //                     true
            //                 } else {
            //                     offset += summary.bytes;
            //                     false
            //                 }
            //             })?
            //             .as_ref();
            //     }
            //     Node::Leaf(leaf) => return Some((offset, leaf)),
            // }
        }
    }

    pub fn items(&self, range: impl RangeBounds<usize>) -> impl Iterator<Item = (usize, Id)> + '_ {
        let (start, end) = (range.start_bound().cloned(), range.end_bound().cloned());
        let mut q = VecDeque::from([(0, self.tree.root().as_ref())]);

        iter::from_coroutine(
            #[coroutine]
            move || {
                while let Some((mut offset, node)) = q.pop_front() {
                    match node {
                        Node::Internal(inode) => {
                            for child in inode.children().iter() {
                                let summary = child.summary();
                                // TODO something like the following logic to avoid adding
                                // unnecessary nodes to the queue
                                // let child_range = (offset..offset + summary.bytes);
                                // if !child_range.contains(start..end) {
                                //     continue;
                                // }

                                q.push_back((offset, child.as_ref()));
                                offset += summary.bytes;
                                if end.gt(&offset) {
                                    break;
                                }
                            }
                        }
                        Node::Leaf(leaf) => {
                            for entry in leaf.as_slice().entries {
                                if end.gt(&offset) {
                                    break;
                                }

                                for id in entry.ids.iter() {
                                    yield (offset, id.into());
                                }

                                offset += entry.length.get();

                                if end.lt(&offset) {
                                    break;
                                }
                            }
                        }
                    }
                }
            },
        )
    }

    /// Inserts an item based on its byte position.
    /// This does not affect `self.len()`.
    pub fn insert(&mut self, at: usize, id: Id) {
        if self.tree.summary().ids.contains(id.into()) {
            todo!("MarkTree insertion of existing id")
        }

        assert!(at <= self.len(), "byte {at} out of bounds of marktree of length {}", self.len());
        self.replace(at..at, Replacement::Id(id.into()))
    }

    pub fn delete(&mut self, id: Id) -> Option<usize> {
        fn del<const N: usize>(
            node: &mut Arc<Node<ARITY, Leaf<N>>>,
            mut offset: usize,
            id: u64,
        ) -> usize {
            match Arc::make_mut(node) {
                Node::Internal(inode) => {
                    for i in 0..inode.children().len() {
                        let summary = inode.child(i).summary();
                        if summary.ids.contains(id.into()) {
                            return inode.with_child_mut(i, |child| del(child, offset, id));
                        }
                        offset += summary.bytes;
                    }
                    unreachable!("bitmaps said it's here")
                }
                Node::Leaf(leaf) => {
                    debug_assert!(leaf.summary().ids.contains(id.into()));
                    leaf.summary_mut().ids.remove(id.into());
                    let leaf_offset = leaf.value_mut().delete(id).expect("bitmap said it's here");
                    offset + leaf_offset
                }
            }
        }

        let root = self.tree.root_mut();
        if !root.summary().ids.contains(id.into()) {
            return None;
        }

        Some(del(root, 0, id.into()))
    }

    /// Applies the given `deltas` to the tree.
    /// This will update the byte positions of the items.
    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        for delta in deltas.iter() {
            let range = delta.range();
            self.shift(range, delta.text().len());
        }
    }

    fn replace(&mut self, range: impl RangeBounds<usize>, replace_with: Replacement) {
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        self.tree.replace(ByteMetric(start)..ByteMetric(end), replace_with);
    }
}

pub struct Drain<'a, Id: MTreeId, M: MTree<Id>> {
    tree: &'a mut M,
    ids: std::vec::IntoIter<Id>,
}

impl<'a, Id: MTreeId, M: MTree<Id>> Drop for Drain<'a, Id, M> {
    fn drop(&mut self) {
        for id in self.ids.by_ref() {
            self.tree.delete(id).unwrap();
        }
    }
}

impl<'a, Id: MTreeId, M: MTree<Id>> Iterator for Drain<'a, Id, M> {
    type Item = (usize, Id);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(|id| {
            let offset = self
                .tree
                .delete(id)
                .expect("id was in the tree, and we're still holding &mut Tree");
            (offset, id)
        })
    }
}

// NOTE: It's important to have a structure such that every leaf entry has a non-zero length.
// Otherwise, a zero-length entry could take up arbitrarily many slots in the tree which breaks assumptions by the tree impl.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LeafEntry {
    /// The length of the entry.
    length: NonZeroUsize,
    /// The ids contained within this range.
    /// All their positions is considered to be the start of the entry.
    ids: tinyset::SetU64,
}

impl LeafEntry {
    fn new(length: usize, ids: impl IntoIterator<Item = u64>) -> Self {
        Self {
            length: NonZeroUsize::new(length).expect("leaf entry length must be non-zero"),
            ids: tinyset::SetU64::from_iter(ids),
        }
    }
}

// A fixed-size sorted array of items.
// Invariants:
//  - The final element is always a `Gap` with a non-zero size
//  - But how to implement this? If there's a bunch of items in a row we have to represent that
//  somehow.
#[derive(Debug, Clone)]
struct Leaf<const N: usize> {
    entries: ArrayVec<LeafEntry, N>,
}

impl<const N: usize> Leaf<N> {
    fn delete(&mut self, id: u64) -> Option<usize> {
        todo!();
    }
}

impl<const N: usize> From<ArrayVec<LeafEntry, N>> for Leaf<N> {
    #[inline]
    fn from(entries: ArrayVec<LeafEntry, N>) -> Self {
        Self { entries }
    }
}

impl<const N: usize> Default for Leaf<N> {
    fn default() -> Self {
        Self::from(ArrayVec::new())
    }
}

impl<const N: usize> From<LeafSlice<'_>> for Leaf<N> {
    #[inline]
    fn from(slice: LeafSlice<'_>) -> Self {
        Self::from(ArrayVec::try_from(slice.entries).unwrap())
    }
}

enum Replacement {
    Id(u64),
    Gap(NonZeroUsize),
    Shift(usize),
}

impl<const N: usize> ReplaceableLeaf<ByteMetric> for Leaf<N> {
    type Replacement<'a> = Replacement;

    type ExtraLeaves = impl ExactSizeIterator<Item = Self>;

    fn replace<R>(
        &mut self,
        summary: &mut Self::Summary,
        range: R,
        replace_with: Self::Replacement<'_>,
    ) -> Option<Self::ExtraLeaves>
    where
        R: RangeBounds<ByteMetric>,
    {
        debug_assert_eq!(*summary, self.summarize());
        let n = summary.bytes;
        let (start, end) = range_bounds_to_start_end(range, 0, n);
        assert!(end <= n, "end <= n ({end} <= {n})");

        if self.entries.is_empty() {
            debug_assert_eq!(n, 0);
            match replace_with {
                Replacement::Id(id) => {
                    self.entries.push(LeafEntry::new(1, iter::once(id)));
                }
                Replacement::Gap(gap) => {
                    self.entries.push(LeafEntry::new(gap.get(), iter::empty()));
                }
                Replacement::Shift(_) => {}
            }

            *summary = self.summarize();
            return None;
        }

        let mut replace_with = Some(replace_with);
        let mut offset = 0;
        let mut new_entries = vec![];

        for entry in self.entries.take() {
            new_entries.push(entry);
        }

        let mut chunks = new_entries.array_chunks::<N>();
        let (chunk, used_remainder) = match chunks.next() {
            Some(chunk) => (ArrayVec::from(chunk.clone()), false),
            None => (
                ArrayVec::try_from(chunks.remainder()).expect("remainder can't be too large"),
                true,
            ),
        };
        self.entries = chunk;

        *summary = self.summarize();

        if chunks.len() == 0 && (used_remainder || chunks.remainder().is_empty()) {
            return None;
        }

        let rem = if chunks.remainder().is_empty() {
            None
        } else {
            Some(ArrayVec::try_from(chunks.remainder()).expect("remainder can't be too large"))
        };

        Some(
            chunks
                .cloned()
                .map(ArrayVec::from)
                .exact_chain(rem)
                .map(Leaf::from)
                // TODO maybe can avoid the collect here
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }

    fn remove_up_to(&mut self, summary: &mut Self::Summary, up_to: ByteMetric) {
        todo!();

        *summary = self.summarize();
    }
}

impl<const N: usize> BalancedLeaf for Leaf<N> {
    // TODO implement
    fn is_underfilled(&self, _summary: &Self::Summary) -> bool {
        false
    }

    fn balance_leaves(
        _left: (&mut Self, &mut Self::Summary),
        _right: (&mut Self, &mut Self::Summary),
    ) {
        todo!()
    }
}

impl<const N: usize> Summarize for Leaf<N> {
    type Summary = Summary;

    fn summarize(&self) -> Self::Summary {
        self.as_slice().summarize()
    }
}

impl<const N: usize> BaseMeasured for Leaf<N> {
    type BaseMetric = ByteMetric;
}

impl<const N: usize> AsSlice for Leaf<N> {
    type Slice<'a> = LeafSlice<'a > where Self: 'a;

    fn as_slice(&self) -> Self::Slice<'_> {
        LeafSlice { entries: &self.entries }
    }
}

#[derive(Debug, Clone, Copy)]
struct LeafSlice<'a> {
    entries: &'a [LeafEntry],
}

impl<'a> Default for LeafSlice<'a> {
    #[inline]
    fn default() -> Self {
        Self { entries: &[] }
    }
}

impl<'a> LeafSlice<'a> {
    /// Return the item with the given `id` if it exists.
    /// The item `byte` is relative to the start of the leaf node.
    fn get(&self, id: u64) -> Option<usize> {
        todo!()
    }
}

impl<'a> Summarize for LeafSlice<'a> {
    type Summary = Summary;

    fn summarize(&self) -> Self::Summary {
        Summary {
            bytes: self.entries.iter().map(|entry| entry.length.get()).sum(),
            ids: RoaringTreemap::from_iter(self.entries.iter().flat_map(|entry| entry.ids.iter())),
        }
    }
}

impl Add<Self> for Summary {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: Self) -> Self {
        self += rhs;
        self
    }
}

impl Sub<Self> for Summary {
    type Output = Self;

    #[inline]
    fn sub(mut self, rhs: Self) -> Self {
        self -= rhs;
        self
    }
}

impl Add<&Self> for Summary {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: &Self) -> Self {
        self += rhs;
        self
    }
}

impl Sub<&Self> for Summary {
    type Output = Self;

    #[inline]
    fn sub(mut self, rhs: &Self) -> Self {
        self -= rhs;
        self
    }
}

impl AddAssign<Self> for Summary {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.bytes += rhs.bytes;
        self.ids |= rhs.ids;
    }
}

impl SubAssign<Self> for Summary {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.bytes -= rhs.bytes;
        self.ids -= rhs.ids;
    }
}

impl AddAssign<&Self> for Summary {
    #[inline]
    fn add_assign(&mut self, rhs: &Self) {
        self.bytes += rhs.bytes;
        self.ids |= &rhs.ids;
    }
}

impl SubAssign<&Self> for Summary {
    #[inline]
    fn sub_assign(&mut self, rhs: &Self) {
        self.bytes -= rhs.bytes;
        self.ids -= &rhs.ids;
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
struct Summary {
    ids: RoaringTreemap,
    bytes: usize,
}

#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
struct ByteMetric(usize);

// NOTE: It would be nice to have the following impl but I don't think it's possible to implement.
// We would need to be able to make arbitrary slices into the `Leaf` which is not possible due to having `Gap`s.
// impl<Id: MTreeId, const N: usize> SlicingMetric<Leaf<T, N>> for ByteMetric {}

impl Metric<Summary> for ByteMetric {
    #[inline]
    fn zero() -> Self {
        Self(0)
    }

    #[inline]
    fn one() -> Self {
        Self(1)
    }

    fn measure(summary: &Summary) -> Self {
        Self(summary.bytes)
    }
}

impl Add<Self> for ByteMetric {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for ByteMetric {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl AddAssign for ByteMetric {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.0 += other.0
    }
}

impl SubAssign for ByteMetric {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.0 -= other.0
    }
}

impl Add<usize> for ByteMetric {
    type Output = usize;

    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        self.0 + rhs
    }
}

impl From<ByteMetric> for usize {
    #[inline]
    fn from(metric: ByteMetric) -> Self {
        metric.0
    }
}

// Below copied from `crop`
#[inline]
fn range_bounds_to_start_end<T, B>(range: B, lo: usize, hi: usize) -> (usize, usize)
where
    B: RangeBounds<T>,
    T: Add<usize, Output = usize> + Into<usize> + Copy,
{
    use std::ops::Bound;

    let start = match range.start_bound() {
        Bound::Included(&n) => n.into(),
        Bound::Excluded(&n) => n + 1,
        Bound::Unbounded => lo,
    };

    let end = match range.end_bound() {
        Bound::Included(&n) => n + 1,
        Bound::Excluded(&n) => n.into(),
        Bound::Unbounded => hi,
    };

    assert!(start >= lo, "start={start} >= lo={lo}");
    assert!(end <= hi, "end={end} <= hi={hi}");

    (start, end)
}

#[cfg(test)]
mod tests {
    // use LeafEntry::*;
    //
    // use super::*;
    //
    // #[test]
    // fn remove_up_to() {
    //     #[track_caller]
    //     fn check<const N: usize>(
    //         iter: impl IntoIterator<Item = LeafEntry<u64>>,
    //         up_to: usize,
    //         expected: impl IntoIterator<Item = LeafEntry<u64>>,
    //         expected_summary: (usize, impl IntoIterator<Item = u64>),
    //     ) {
    //         let mut leaf = Leaf::<u64, N>::from(ArrayVec::from_iter(iter));
    //         let mut summary = leaf.summarize();
    //         leaf.remove_up_to(&mut summary, ByteMetric(up_to));
    //         assert_eq!(leaf.entries, ArrayVec::from_iter(expected));
    //         let expected_summary = Summary {
    //             bytes: expected_summary.0,
    //             ids: RoaringTreemap::from_iter(expected_summary.1),
    //         };
    //         assert_eq!(summary, expected_summary);
    //     }
    //
    //     check::<4>([Item(0), Gap(1), Item(1)], 1, [], (0, []));
    //     check::<10>([Item(0), Gap(1), Item(1), Gap(1), Item(2)], 1, [Gap(1), Item(2)], (1, [2]));
    //     check::<10>([Gap(1), Item(0), Gap(1), Item(1)], 1, [Gap(1), Item(1)], (1, [1]));
    // }
}
