use std::ops::{Add, AddAssign, RangeBounds, Sub, SubAssign};
use std::{fmt, iter};

use arrayvec::ArrayVec;
use crop::tree::{
    AsSlice, BalancedLeaf, BaseMeasured, Lnode, Metric, Node, ReplaceableLeaf, Summarize, Tree,
};
use roaring::RoaringTreemap;
use stdx::iter::ExactChain;

use crate::Deltas;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Bias {
    // Important that `Self::Left < Self::Right`
    #[default]
    Left,
    Right,
}

const ARITY: usize = 4;

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
pub struct MarkTree<T: MarkTreeItem, const N: usize> {
    tree: Tree<ARITY, Leaf<T, N>>,
}

pub trait MarkTreeItem: fmt::Debug + Clone + 'static {
    type Id: Eq + Copy + Into<u64>;

    /// The `id` of the item.
    fn id(&self) -> Self::Id;

    /// The byte position of the item.
    fn byte(&self) -> usize;

    /// Returns a new item with the same data at the given byte position.
    fn at(&self, byte: usize) -> Self;
}

// (byte, id)
// Makes sense for the `byte` to come first as it determines the order.
impl<I: Eq + Copy + Into<u64> + fmt::Debug + 'static> MarkTreeItem for (usize, I) {
    type Id = I;

    #[inline]
    fn byte(&self) -> usize {
        self.0
    }

    #[inline]
    fn id(&self) -> Self::Id {
        self.1
    }

    #[inline]
    fn at(&self, byte: usize) -> Self {
        (byte, self.1)
    }
}

impl<const N: usize, T: MarkTreeItem> MarkTree<T, N> {
    /// Creates a new `MarkTree` with a single gap of `n` bytes.
    /// This should be equal to the length of the text in bytes.
    pub fn new(n: usize) -> Self {
        let mut this = Self { tree: Tree::default() };
        this.replace(0..0, LeafEntry::Gap(n));
        this
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tree.summary().bytes
    }

    pub fn get(&self, id: T::Id) -> Option<T> {
        let (offset, leaf) = self.find(id)?;
        let item = leaf.as_slice().get(id)?;
        Some(item.at(offset + item.byte()))
    }

    fn find(&self, id: T::Id) -> Option<(usize, &Lnode<Leaf<T, N>>)> {
        let raw_id = id.into();
        // Need to do a manual traversal to make use of the bitmaps.
        let mut node = self.tree.root().as_ref();
        if !node.summary().ids.contains(raw_id) {
            return None;
        }

        let mut offset = 0;

        loop {
            debug_assert!(node.summary().ids.contains(raw_id));
            match node {
                Node::Internal(inode) => {
                    node = inode
                        .children()
                        .iter()
                        .find(|child| {
                            let summary = child.summary();
                            if summary.ids.contains(raw_id) {
                                true
                            } else {
                                offset += summary.bytes;
                                false
                            }
                        })?
                        .as_ref();
                }
                Node::Leaf(leaf) => return Some((offset, leaf)),
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        let leaves = self.tree.leaves();
        let mut shift = 0;
        iter::from_coroutine(
            #[coroutine]
            move || {
                for leaf in leaves {
                    for entry in leaf.entries {
                        match entry {
                            LeafEntry::Gap(n) => shift += n,
                            LeafEntry::Item(item) => yield item.at(shift),
                        }
                    }
                }
            },
        )
    }

    /// Inserts an item based on its byte position.
    /// This does not affect `self.len()`.
    pub fn insert(&mut self, item: T) {
        if self.tree.summary().ids.contains(item.id().into()) {
            todo!("MarkTree insertion of existing id")
        }

        let byte = item.byte();
        assert!(
            byte < self.len(),
            "byte {byte} out of bounds of marktree of length {}",
            self.len()
        );
        self.replace(byte..byte, LeafEntry::Item(item))
    }

    pub fn delete(&mut self, id: T::Id) -> Option<T> {
        let item = self.get(id)?;
        // Delete item by range somehow

        todo!()
    }

    /// Clear the marks in the given range.
    /// This does not affect `self.len()`;
    pub fn clear_range(&mut self, range: impl RangeBounds<usize>) {
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        self.replace(start..end, LeafEntry::Gap(end - start));
    }

    /// Applies the given `deltas` to the tree.
    /// This will update the byte positions of the items.
    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        for delta in deltas.iter() {
            let range = delta.range();
            self.replace(range, LeafEntry::Gap(delta.text().len()));
        }
    }

    fn replace(&mut self, range: impl RangeBounds<usize>, replace_with: LeafEntry<T>) {
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        self.tree.replace(ByteMetric(start)..ByteMetric(end), replace_with);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LeafEntry<T> {
    Item(T),
    Gap(usize),
}

// A fixed-size sorted array of items.
#[derive(Debug, Clone)]
struct Leaf<T: MarkTreeItem, const N: usize> {
    entries: ArrayVec<LeafEntry<T>, N>,
}

impl<T: MarkTreeItem, const N: usize> From<ArrayVec<LeafEntry<T>, N>> for Leaf<T, N> {
    #[inline]
    fn from(entries: ArrayVec<LeafEntry<T>, N>) -> Self {
        Self { entries }
    }
}

impl<T: MarkTreeItem, const N: usize> Default for Leaf<T, N> {
    fn default() -> Self {
        Self::from(ArrayVec::new())
    }
}

impl<T: MarkTreeItem, const N: usize> From<LeafSlice<'_, T>> for Leaf<T, N> {
    #[inline]
    fn from(slice: LeafSlice<'_, T>) -> Self {
        Self::from(ArrayVec::try_from(slice.entries).unwrap())
    }
}

impl<T: MarkTreeItem, const N: usize> ReplaceableLeaf<ByteMetric> for Leaf<T, N> {
    type Replacement<'a> = LeafEntry<T>;

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
            self.entries.push(replace_with);
            *summary = self.summarize();
            return None;
        }

        let mut replace_with = Some(replace_with);

        // naive algorithm for now
        // rebuild `self` with the new entry spliced in appropriately
        // Splitting is also done naively by collecting into a vec and then splitting into arrays
        #[derive(Debug, PartialEq, Eq)]
        enum State {
            Start,
            Skipping { skipped: usize },
            Copy,
        }

        #[derive(Debug)]
        struct EntryBuilder<T> {
            offset: usize,
            entries: Vec<LeafEntry<T>>,
        }

        impl<T> Default for EntryBuilder<T> {
            fn default() -> Self {
                Self { offset: 0, entries: Vec::new() }
            }
        }

        impl<T: MarkTreeItem> EntryBuilder<T> {
            fn push(&mut self, entry: LeafEntry<T>) {
                let entry = match entry {
                    LeafEntry::Item(item) => LeafEntry::Item(item.at(self.offset)),
                    LeafEntry::Gap(gap) => {
                        if gap == 0 {
                            return;
                        }

                        self.offset += gap;

                        if let Some(LeafEntry::Gap(last_gap)) = self.entries.last_mut() {
                            *last_gap += gap;
                            return;
                        }

                        LeafEntry::Gap(gap)
                    }
                };

                self.entries.push(entry);
            }
        }

        use State::*;

        // let mut state = if start == 0 { Skipping { skipped: 0 } } else { Start };
        let mut state = Start;
        let mut builder = EntryBuilder::default();

        for entry in self.entries.take() {
            match entry {
                LeafEntry::Item(item) if !matches!(state, Skipping { .. }) => {
                    let byte = builder.offset;
                    if byte < start || byte >= end {
                        // This condition can be false if the first entry is an item and the start is 0.
                        builder.push(LeafEntry::Item(item))
                    }
                }
                LeafEntry::Item(_) => {}
                LeafEntry::Gap(gap) => {
                    match state {
                        Start => {
                            if builder.offset + gap > start {
                                // We've passed the start of the replacement.
                                // Keep the gap until the replacement starts.
                                // Skip until the end of the replacement.
                                let partial_gap = start - builder.offset;
                                builder.push(LeafEntry::Gap(partial_gap));

                                state = Skipping { skipped: gap - partial_gap };

                                // If this entry covers the entire replacement, we're done.
                                if builder.offset + gap >= end {
                                    let remaining_gap = builder.offset + gap - end - partial_gap;
                                    match replace_with.take().expect("used replacement twice") {
                                        LeafEntry::Item(item) => {
                                            builder.push(LeafEntry::Item(item));
                                            builder.push(LeafEntry::Gap(remaining_gap));
                                        }
                                        LeafEntry::Gap(gap) => {
                                            builder.push(LeafEntry::Gap(gap + remaining_gap))
                                        }
                                    }
                                    state = Copy;
                                }
                            } else {
                                builder.push(LeafEntry::Gap(gap));
                            }
                        }
                        Skipping { skipped } => {
                            if builder.offset + gap >= end {
                                // We've passed the end of the replacement.
                                // Keep the rest of the gap until the end of the skip.
                                builder
                                    .push(LeafEntry::Gap(skipped + gap - (end - builder.offset)));
                                let replacement =
                                    replace_with.take().expect("used replacement twice");
                                match replacement {
                                    LeafEntry::Item(item) => builder.push(LeafEntry::Item(item)),
                                    LeafEntry::Gap(gap) => builder.push(LeafEntry::Gap(gap)),
                                }

                                state = Copy;
                            } else {
                                state = Skipping { skipped: skipped + gap };
                            }
                        }
                        Copy => builder.push(LeafEntry::Gap(gap)),
                    }
                }
            }
        }

        if let Some(replace_with) = replace_with {
            builder.push(replace_with);
        }

        let mut chunks = builder.entries.array_chunks::<N>();
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
        let ByteMetric(up_to) = up_to;

        assert!(up_to <= summary.bytes);
        let mut offset = 0;

        *self = Self {
            entries: self
                .entries
                .take()
                .into_iter()
                .filter_map(|entry| match entry {
                    LeafEntry::Item(item) => {
                        (offset > up_to).then(|| LeafEntry::Item(item.at(item.byte() - up_to)))
                    }
                    LeafEntry::Gap(gap) if offset + gap <= up_to => {
                        offset += gap;
                        None
                    }
                    LeafEntry::Gap(gap) if offset < up_to && offset + gap > up_to => {
                        // We know that `summary.bytes + gap >= up_to`.
                        let remaining_gap = offset + gap - up_to;
                        offset = up_to;

                        if remaining_gap > 0 { Some(LeafEntry::Gap(remaining_gap)) } else { None }
                    }
                    LeafEntry::Gap(gap) => {
                        offset += gap;
                        Some(entry)
                    }
                })
                .collect(),
        };

        *summary = self.summarize();
    }
}

impl<T: MarkTreeItem, const N: usize> BalancedLeaf for Leaf<T, N> {
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

impl<T: MarkTreeItem, const N: usize> Summarize for Leaf<T, N> {
    type Summary = Summary;

    fn summarize(&self) -> Self::Summary {
        self.as_slice().summarize()
    }
}

impl<T: MarkTreeItem, const N: usize> BaseMeasured for Leaf<T, N> {
    type BaseMetric = ByteMetric;
}

impl<T: MarkTreeItem, const N: usize> AsSlice for Leaf<T, N> {
    type Slice<'a> = LeafSlice<'a,  T> where Self: 'a;

    fn as_slice(&self) -> Self::Slice<'_> {
        LeafSlice { entries: &self.entries }
    }
}

#[derive(Debug, Clone)]
struct LeafSlice<'a, T: MarkTreeItem> {
    entries: &'a [LeafEntry<T>],
}

impl<'a, T: MarkTreeItem> LeafSlice<'a, T> {
    /// Return the item with the given `id` if it exists.
    /// The item `byte` is relative to the start of the leaf node.
    fn get(&self, id: T::Id) -> Option<T> {
        let mut offset = 0;
        self.entries.iter().find_map(|entry| match entry {
            LeafEntry::Item(item) if item.id() == id => Some(item.at(offset)),
            LeafEntry::Item(_) => None,
            LeafEntry::Gap(gap) => {
                offset += *gap;
                None
            }
        })
    }
}

impl<'a, T: MarkTreeItem> Copy for LeafSlice<'a, T> {}

impl<'a, T: MarkTreeItem> Summarize for LeafSlice<'a, T> {
    type Summary = Summary;

    fn summarize(&self) -> Self::Summary {
        let mut ids = RoaringTreemap::new();
        let bytes = self
            .entries
            .iter()
            .map(|entry| match entry {
                LeafEntry::Item(item) => {
                    ids.insert(item.id().into());
                    0
                }
                LeafEntry::Gap(n) => *n,
            })
            .sum();

        Summary { bytes, ids }
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
        *self += &rhs;
    }
}

impl SubAssign<Self> for Summary {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self -= &rhs;
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

    (start, end)
}

#[cfg(test)]
mod tests {
    use LeafEntry::*;

    use super::*;

    #[test]
    fn remove_up_to() {
        #[track_caller]
        fn check<const N: usize>(
            iter: impl IntoIterator<Item = LeafEntry<(usize, u64)>>,
            up_to: usize,
            expected: impl IntoIterator<Item = LeafEntry<(usize, u64)>>,
            expected_summary: (usize, impl IntoIterator<Item = u64>),
        ) {
            let mut leaf = Leaf::<(usize, u64), N>::from(ArrayVec::from_iter(iter));
            let mut summary = leaf.summarize();
            leaf.remove_up_to(&mut summary, ByteMetric(up_to));
            assert_eq!(leaf.entries, ArrayVec::from_iter(expected));
            let expected_summary = Summary {
                bytes: expected_summary.0,
                ids: RoaringTreemap::from_iter(expected_summary.1),
            };
            assert_eq!(summary, expected_summary);
        }

        check::<4>([Item((0, 0)), Gap(1), Item((1, 1))], 1, [], (0, []));

        check::<10>(
            [Item((0, 0)), Gap(1), Item((1, 1)), Gap(1), Item((2, 2))],
            1,
            [Gap(1), Item((1, 2))],
            (1, [2]),
        );

        check::<10>(
            [Gap(1), Item((1, 0)), Gap(1), Item((2, 1))],
            1,
            [Gap(1), Item((1, 1))],
            (1, [1]),
        );
    }
}
