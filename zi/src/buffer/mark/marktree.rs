use std::fmt;
use std::mem::MaybeUninit;
use std::ops::{Add, AddAssign, RangeBounds, Sub, SubAssign};

use crop::tree::{AsSlice, BalancedLeaf, BaseMeasured, Metric, ReplaceableLeaf, Summarize};

pub(super) struct MarkTree<const N: usize, T: Item> {
    tree: crop::tree::Tree<4, Leaf<N, T>>,
}

pub trait Item: fmt::Debug + Copy + 'static {
    fn byte(&self) -> usize;
}

impl Item for usize {
    #[inline]
    fn byte(&self) -> usize {
        *self
    }
}

impl<const N: usize, T: Item> Default for MarkTree<N, T> {
    fn default() -> Self {
        Self { tree: crop::tree::Tree::default() }
    }
}

impl<const N: usize, T: Item> MarkTree<N, T> {
    pub fn insert(&mut self, item: T) {
        todo!()
    }
}

#[derive(Debug)]
struct Leaf<const N: usize, T> {
    data: [MaybeUninit<T>; N],
}

impl<const N: usize, T> Default for Leaf<N, T> {
    fn default() -> Self {
        Self { data: MaybeUninit::uninit_array() }
    }
}

impl<const N: usize, T: Item> From<LeafSlice<'_, N, T>> for Leaf<N, T> {
    fn from(slice: LeafSlice<'_, N, T>) -> Self {
        Self { data: *slice.data }
    }
}

impl<const N: usize, T: Item> ReplaceableLeaf<ByteMetric> for Leaf<N, T> {
    type Replacement<'a> = &'a [u8];

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
        None
    }

    fn remove_up_to(&mut self, summary: &mut Self::Summary, up_to: ByteMetric) {
        todo!()
    }
}

impl<const N: usize, T: Item> BalancedLeaf for Leaf<N, T> {
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

impl<const N: usize, T: Item> Summarize for Leaf<N, T> {
    type Summary = Summary;

    fn summarize(&self) -> Self::Summary {
        todo!()
    }
}

impl<const N: usize, T: Item> BaseMeasured for Leaf<N, T> {
    type BaseMetric = ByteMetric;
}

impl<const N: usize, T: Item> AsSlice for Leaf<N, T> {
    type Slice<'a> = LeafSlice<'a, N, T> where Self: 'a;

    fn as_slice(&self) -> Self::Slice<'_> {
        todo!()
    }
}

#[derive(Debug, Clone, Copy)]
struct LeafSlice<'a, const N: usize, T: Item> {
    data: &'a [MaybeUninit<T>; N],
}

impl<'a, const N: usize, T: Item> Summarize for LeafSlice<'a, N, T> {
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
