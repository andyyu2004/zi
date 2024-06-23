use std::ops::{Add, AddAssign, RangeBounds, Sub, SubAssign};
use std::{fmt, iter};

use arrayvec::ArrayVec;
use crop::tree::{AsSlice, BalancedLeaf, BaseMeasured, Metric, ReplaceableLeaf, Summarize, Tree};
use stdx::iter::ExactChain;

use crate::Deltas;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Bias {
    // Important `Left < Right`
    #[default]
    Left,
    Right,
}

slotmap::new_key_type! {
    pub struct MarkId;
}

const ARITY: usize = 4;

#[derive(Debug)]
pub struct MarkTree<T: MarkTreeItem, const N: usize> {
    tree: Tree<ARITY, Leaf<T, N>>,
}

pub trait MarkTreeItem: fmt::Debug + Clone + 'static {
    fn byte(&self) -> usize;

    fn at(&self, byte: usize) -> Self;
}

impl MarkTreeItem for usize {
    #[inline]
    fn byte(&self) -> usize {
        *self
    }

    #[inline]
    #[track_caller]
    fn at(&self, byte: usize) -> Self {
        byte
    }
}

impl<const N: usize, T: MarkTreeItem> MarkTree<T, N> {
    /// Creates a new `MarkTree` with a single gap of `n` bytes.
    /// This should be equal to the length of the text in bytes.
    pub fn new(n: usize) -> Self {
        let mut this = Self { tree: Tree::default() };
        this.replace_(0..0, LeafEntry::Gap(n));
        this
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tree.summary().bytes
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

    pub fn insert(&mut self, item: T) {
        let byte = item.byte();
        self.tree.replace(ByteMetric(byte)..ByteMetric(byte), LeafEntry::Item(item))
    }

    pub fn replace(&mut self, range: impl RangeBounds<usize>, replace_with: T) {
        self.replace_(range, LeafEntry::Item(replace_with));
    }

    fn replace_(&mut self, range: impl RangeBounds<usize>, replace_with: LeafEntry<T>) {
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        self.tree.replace(ByteMetric(start)..ByteMetric(end), replace_with);
    }

    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        for delta in deltas.iter() {
            let range = delta.range();
            self.replace_(range, LeafEntry::Gap(delta.text().len()));
        }
    }
}

#[derive(Debug, Clone)]
enum LeafEntry<T> {
    Item(T),
    Gap(usize),
}

// A fixed-size sorted array of items.
#[derive(Debug, Clone)]
struct Leaf<T: MarkTreeItem, const N: usize> {
    entries: ArrayVec<LeafEntry<T>, N>,
}

impl<T: MarkTreeItem, const N: usize> Default for Leaf<T, N> {
    fn default() -> Self {
        Self { entries: ArrayVec::new() }
    }
}

impl<T: MarkTreeItem, const N: usize> From<LeafSlice<'_, T>> for Leaf<T, N> {
    #[inline]
    fn from(slice: LeafSlice<'_, T>) -> Self {
        Self { entries: ArrayVec::try_from(slice.entries).unwrap() }
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
        enum State {
            Start,
            Skipping,
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

        let mut state = State::Start;
        let mut builder = EntryBuilder::default();

        for entry in self.entries.take() {
            match entry {
                LeafEntry::Item(item) if !matches!(state, Skipping) => {
                    builder.push(LeafEntry::Item(item))
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

                                state = Skipping;

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
                        Skipping => {
                            if builder.offset + gap >= end {
                                // We've passed the end of the replacement.
                                // Keep the gap until the end of the gap.
                                builder.push(LeafEntry::Gap(gap - (end - builder.offset)));
                                let replacement =
                                    replace_with.take().expect("used replacement twice");
                                match replacement {
                                    LeafEntry::Item(item) => {
                                        builder.push(LeafEntry::Item(item));
                                    }
                                    LeafEntry::Gap(_gap) => {
                                        todo!();
                                        // builder.push(LeafEntry::Gap(gap));
                                    }
                                }

                                state = Copy;
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
        self.entries = match chunks.next() {
            Some(chunk) => ArrayVec::from(chunk.clone()),
            None => ArrayVec::try_from(chunks.remainder()).expect("remainder can't be too large"),
        };

        *summary = self.summarize();

        if chunks.len() == 0 {
            None
        } else {
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
                    .map(|entries| Leaf { entries })
                    // TODO maybe can avoid the collect here
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        }
    }

    fn remove_up_to(&mut self, _summary: &mut Self::Summary, _up_to: ByteMetric) {
        todo!()
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

impl<'a, T: MarkTreeItem> Copy for LeafSlice<'a, T> {}

impl<'a, T: MarkTreeItem> Summarize for LeafSlice<'a, T> {
    type Summary = Summary;

    fn summarize(&self) -> Self::Summary {
        let bytes = self
            .entries
            .iter()
            .map(|entry| match entry {
                LeafEntry::Item(_) => 0,
                LeafEntry::Gap(n) => *n,
            })
            .sum();
        Summary { bytes }
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
