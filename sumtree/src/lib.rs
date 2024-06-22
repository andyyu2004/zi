#![feature(iter_from_coroutine, coroutines)]

//! A generalization of a rope.

use std::ops::{Add, AddAssign, Range, RangeBounds, Sub, SubAssign};
use std::{fmt, iter};

use arrayvec::ArrayVec;
use crop::tree::{
    AsSlice, BalancedLeaf, BaseMeasured, Leaves, Metric, ReplaceableLeaf, Summarize, Tree,
};

const ARITY: usize = 4;

pub struct MarkTree<T: Item, const N: usize> {
    tree: Tree<ARITY, Leaf<T, N>>,
}

pub trait Item: fmt::Debug + Clone + 'static {
    fn byte(&self) -> usize;
}

impl Item for usize {
    #[inline]
    fn byte(&self) -> usize {
        *self
    }
}

impl<const N: usize, T: Item> MarkTree<T, N> {
    pub fn new(n: usize) -> Self {
        let mut this = Self { tree: Tree::default() };
        this.tree.replace(ByteMetric(0)..ByteMetric(0), LeafEntry::Gap(n));
        this
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let leaves = self.tree.leaves();
        iter::from_coroutine(
            #[coroutine]
            || {
                for leaf in leaves {
                    for entry in leaf.data {
                        if let LeafEntry::Item(item) = entry {
                            yield item;
                        }
                    }
                }
            },
        )
    }

    pub fn insert(&mut self, item: T) {
        let byte = item.byte();
        self.tree.replace(ByteMetric(byte)..ByteMetric(byte), LeafEntry::Item(item))
    }

    // tmp approx of Delta
    pub fn shift(&mut self, range: Range<usize>, shift: usize) {}
}

#[derive(Debug, Clone, Copy)]
enum LeafEntry<T> {
    Item(T),
    Gap(usize),
}

// A fixed-size sorted array of items.
#[derive(Debug, Clone)]
struct Leaf<T: Item, const N: usize> {
    entries: ArrayVec<LeafEntry<T>, N>,
}

impl<T: Item, const N: usize> Leaf<T, N> {
    #[inline]
    fn len(&self) -> usize {
        // TODO cache this computation
        self.entries
            .iter()
            .map(|entry| match entry {
                LeafEntry::Item(_) => 0,
                LeafEntry::Gap(n) => *n,
            })
            .sum()
    }
}

impl<T: Item, const N: usize> Default for Leaf<T, N> {
    fn default() -> Self {
        Self { entries: ArrayVec::new() }
    }
}

impl<T: Item, const N: usize> From<LeafSlice<'_, T>> for Leaf<T, N> {
    #[inline]
    fn from(slice: LeafSlice<'_, T>) -> Self {
        Self { entries: ArrayVec::try_from(slice.data).unwrap() }
    }
}

impl<T: Item, const N: usize> ReplaceableLeaf<ByteMetric> for Leaf<T, N> {
    type Replacement<'a> = LeafEntry<T>;

    type ExtraLeaves = std::iter::Empty<Self>;

    fn replace<R>(
        &mut self,
        summary: &mut Self::Summary,
        range: R,
        replace_with: Self::Replacement<'_>,
    ) -> Option<Self::ExtraLeaves>
    where
        R: RangeBounds<ByteMetric>,
    {
        let n = self.len();
        let (start, end) = range_bounds_to_start_end(range, 0, n);
        assert!(end <= n, "end <= n ({end} <= {n})");

        if self.entries.is_empty() {
            self.entries.push(replace_with);
            return None;
        }

        let mut replace_with = Some(replace_with);

        // naive algorithm for now
        // rebuild `self` with the new entry
        enum State {
            Start,
            Skipping,
            Copy,
        }

        use State::*;

        let mut state = State::Start;
        let mut entries = ArrayVec::new();
        let mut k = 0;

        // TODO Need to handle splitting
        for entry in &self.entries {
            match entry {
                LeafEntry::Item(item) if !matches!(state, Skipping) => {
                    entries.push(LeafEntry::Item(item.clone()))
                }
                LeafEntry::Item(_) => {}
                LeafEntry::Gap(offset) => {
                    match state {
                        Start => {
                            if k + *offset > start {
                                // We've passed the start of the replacement.
                                // Keep the gap until the replacement starts.
                                // Skip until the end of the replacement.
                                if start - k > 0 {
                                    entries.push(LeafEntry::Gap(start - k));
                                }
                                state = Skipping;

                                // If this entry covers the entire replacement, we're done.
                                if k + *offset >= end {
                                    entries
                                        .push(replace_with.take().expect("used replacement twice"));
                                    // `- 0`` is to consider the `replacement` length which is currently always 0
                                    entries.push(LeafEntry::Gap(k + *offset - end - 0));
                                    state = Copy;
                                }
                            } else {
                                entries.push(LeafEntry::Gap(*offset));
                            }
                        }
                        Skipping => {
                            if k + *offset >= end {
                                // We've passed the end of the replacement.
                                // Keep the gap until the end of the gap.
                                entries.push(LeafEntry::Gap(*offset - (end - k)));
                                entries.push(replace_with.take().expect("used replacement twice"));
                                state = Copy;
                            }
                        }
                        Copy => entries.push(LeafEntry::Gap(*offset)),
                    }

                    k += *offset;
                }
            }
        }

        self.entries = entries;
        *summary = Summary { bytes: self.len() };

        None
    }

    fn remove_up_to(&mut self, summary: &mut Self::Summary, up_to: ByteMetric) {
        todo!()
    }
}

impl<T: Item, const N: usize> BalancedLeaf for Leaf<T, N> {
    fn is_underfilled(&self, summary: &Self::Summary) -> bool {
        todo!()
    }

    fn balance_leaves(
        left: (&mut Self, &mut Self::Summary),
        right: (&mut Self, &mut Self::Summary),
    ) {
        todo!()
    }
}

impl<T: Item, const N: usize> Summarize for Leaf<T, N> {
    type Summary = Summary;

    fn summarize(&self) -> Self::Summary {
        todo!()
    }
}

impl<T: Item, const N: usize> BaseMeasured for Leaf<T, N> {
    type BaseMetric = ByteMetric;
}

impl<T: Item, const N: usize> AsSlice for Leaf<T, N> {
    type Slice<'a> = LeafSlice<'a,  T> where Self: 'a;

    fn as_slice(&self) -> Self::Slice<'_> {
        LeafSlice { data: &self.entries }
    }
}

#[derive(Debug, Clone)]
struct LeafSlice<'a, T: Item> {
    data: &'a [LeafEntry<T>],
}

impl<'a, T: Item> Copy for LeafSlice<'a, T> {}

impl<'a, T: Item> Summarize for LeafSlice<'a, T> {
    type Summary = Summary;

    fn summarize(&self) -> Self::Summary {
        todo!()
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
        // self.line_breaks += rhs.line_breaks;
    }
}

impl SubAssign<Self> for Summary {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.bytes -= rhs.bytes;
        // self.line_breaks -= rhs.line_breaks;
    }
}

impl AddAssign<&Self> for Summary {
    #[inline]
    fn add_assign(&mut self, rhs: &Self) {
        *self += *rhs;
    }
}

impl SubAssign<&Self> for Summary {
    #[inline]
    fn sub_assign(&mut self, rhs: &Self) {
        *self -= *rhs;
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Summary {
    bytes: usize,
    // line_breaks: usize,
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

#[inline]
pub(crate) fn range_bounds_to_start_end<T, B>(range: B, lo: usize, hi: usize) -> (usize, usize)
where
    B: core::ops::RangeBounds<T>,
    T: core::ops::Add<usize, Output = usize> + Into<usize> + Copy,
{
    use core::ops::Bound;

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
