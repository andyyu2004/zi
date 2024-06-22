//! A generalization of a rope.

use std::mem::MaybeUninit;
use std::ops::{Add, AddAssign, RangeBounds, Sub, SubAssign};
use std::{fmt, ops};

use arrayvec::ArrayVec;
use crop::tree::{
    AsSlice, BalancedLeaf, BaseMeasured, Leaves, Metric, ReplaceableLeaf, Summarize, Tree,
};

const ARITY: usize = 4;

pub struct SumTree<T: Item, const N: usize> {
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

impl<const N: usize, T: Item> Default for SumTree<T, N> {
    fn default() -> Self {
        Self { tree: crop::tree::Tree::default() }
    }
}

impl<const N: usize, T: Item> SumTree<T, N> {
    pub fn chunks(&self) -> impl ExactSizeIterator<Item = &[T]> {
        Chunks { leaves: self.tree.leaves() }
    }

    pub fn replace(&mut self, range: ops::Range<usize>, item: T) {
        self.tree.replace(ByteMetric(range.start)..ByteMetric(range.end), item)
    }
}

// A fixed-size sorted array of items.
// `self.data[..self.len]` is the sorted array. The rest is uninitialized.
#[derive(Debug, Clone)]
struct Leaf<T: Item, const N: usize> {
    data: ArrayVec<T, N>,
}

impl<T: Item, const N: usize> Leaf<T, N> {
    #[inline]
    fn len(&self) -> usize {
        self.data.len()
    }
}

impl<T: Item, const N: usize> Default for Leaf<T, N> {
    fn default() -> Self {
        Self { data: ArrayVec::new() }
    }
}

impl<T: Item, const N: usize> From<LeafSlice<'_, T>> for Leaf<T, N> {
    #[inline]
    fn from(slice: LeafSlice<'_, T>) -> Self {
        Self { data: ArrayVec::try_from(slice.data).unwrap() }
    }
}

impl<T: Item, const N: usize> ReplaceableLeaf<ByteMetric> for Leaf<T, N> {
    type Replacement<'a> = T;

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
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        dbg!(start, end);

        if self.len() - (end - start) + 1 <= N { todo!("1") } else { todo!() }
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
        LeafSlice { data: &self.data }
    }
}

#[derive(Debug, Clone)]
struct LeafSlice<'a, T: Item> {
    data: &'a [T],
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
        self.line_breaks += rhs.line_breaks;
    }
}

impl SubAssign<Self> for Summary {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.bytes -= rhs.bytes;
        self.line_breaks -= rhs.line_breaks;
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
    line_breaks: usize,
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

pub struct Chunks<'a, T: Item, const N: usize> {
    leaves: Leaves<'a, ARITY, Leaf<T, N>>,
}

impl<'a, T: Item, const N: usize> Iterator for Chunks<'a, T, N> {
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.leaves.next().map(|leaf| leaf.data)
    }
}

impl<'a, T: Item, const N: usize> ExactSizeIterator for Chunks<'a, T, N> {
    #[inline]
    fn len(&self) -> usize {
        self.leaves.len()
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
