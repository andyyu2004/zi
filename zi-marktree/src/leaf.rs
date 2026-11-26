use std::cmp;
use std::fmt;
use std::ops::{Range, RangeBounds};

use arrayvec::ArrayVec;
use crop::tree::{AsSlice, BalancedLeaf, BaseMeasured, ReplaceableLeaf, Summarize};
use smallvec::SmallVec;
use tinyset::SetU64;

use crate::extent::{Extent, ExtentBuilder};
use crate::key::{Flags, Key};
use crate::summary::{ByteMetric, Summary};
use crate::{Replacement, range_bounds_to_start_end};

#[derive(Clone)]
pub(crate) struct Leaf<const N: usize> {
    extents: ArrayVec<Extent, N>,
}

impl<const N: usize> fmt::Debug for Leaf<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.extents.iter()).finish()
    }
}

impl<const N: usize> Leaf<N> {
    #[inline]
    pub(crate) fn get_left(&self, id: u64) -> Option<usize> {
        self.as_slice().get_left(id)
    }

    #[inline]
    pub(crate) fn get_right(&self, id: u64) -> Option<usize> {
        self.as_slice().get_right(id)
    }

    pub(crate) fn delete(&mut self, summary: &mut Summary, id: u64) -> Option<usize> {
        let mut offset = 0;

        for i in 0..self.extents.len() {
            if self.extents[i].remove(id) {
                assert!(summary.ids.remove(id).is_some());
                if i > 0 && self.extents[i].keys.is_empty() {
                    let Extent { length, .. } = self.extents.remove(i);
                    self.extents[i - 1].length += length;
                }
                return Some(offset);
            }
            offset += self.extents[i].len();
        }

        None
    }

    /// Keeps only the keys for which `f(offset, key)` returns true, where `offset` is the key's
    /// position relative to the start of the leaf.
    pub(crate) fn retain(&mut self, summary: &mut Summary, mut f: impl FnMut(usize, Key) -> bool) {
        let mut builder = ExtentBuilder::<N>::default();
        let mut offset = 0;
        for extent in self.extents.take() {
            builder.push(extent.len(), extent.keys().filter(|&key| f(offset, key)));
            offset += extent.len();
        }
        self.extents = builder.finish().into_iter().collect();
        *summary = self.summarize();
    }

    fn shift(
        &mut self,
        summary: &mut Summary,
        range: Range<usize>,
        by: usize,
    ) -> Option<<Self as ReplaceableLeaf<ByteMetric>>::ExtraLeaves> {
        let (start, end) = (range.start, range.end);

        let mut builder = ExtentBuilder::<N>::default();
        let mut keys = SetU64::default();
        let mut gap = Some(by);

        let mut offset = 0;
        for extent in self.extents.take() {
            let extent_end = offset + extent.len();
            let k = extent.len();

            if extent_end < start || offset > end {
                // If the extent range does not intersect the replacement range just copy
                builder.push_extent(&extent);
                offset += k;

                continue;
            }

            keys = keys | &extent.keys;

            match start.cmp(&offset) {
                cmp::Ordering::Greater => {
                    // The offset is before the start of the replacement range.
                    // Copy the chunk of the extent that precedes the replacement range.
                    builder.push_raw(start - offset, keys.drain());
                }
                cmp::Ordering::Equal => {
                    // The interval starts exactly at offset, move it to the right by pushing the gap first.
                    let gap = gap.take().unwrap();
                    if gap > 0 {
                        let (left_biased, right_biased) =
                            keys.drain().partition::<SetU64, _>(|&key| {
                                Key::from_raw(key).flags().contains(Flags::BIAS_LEFT)
                            });

                        keys = right_biased;

                        builder.push_raw(gap, left_biased);
                    }
                }
                cmp::Ordering::Less => {
                    if let Some(gap) = gap.take() {
                        builder.push_gap(gap);
                    }
                }
            }

            if extent_end > end {
                if let Some(gap) = gap.take() {
                    builder.push_gap(gap);
                }
                // If the extent extends beyond the replacement,
                // push the remaining (right-biased only?) keys after.
                builder.push_raw(extent_end - end, keys.drain());
            }

            offset += k;
        }

        if let Some(gap) = gap {
            builder.push_raw(gap, keys);
        } else if !keys.is_empty() {
            builder.push_raw(0, keys);
        }

        let mut entries = builder.finish();
        if entries.is_empty() {
            // A keyless leaf that was deleted whole. It's underfilled, so `crop` merges it away.
            entries.push(Extent::new(0, []));
        }
        self.set_extents(summary, entries)
    }

    fn insert(
        &mut self,
        summary: &mut Summary,
        at: usize,
        keys: &[Key],
    ) -> Option<<Self as ReplaceableLeaf<ByteMetric>>::ExtraLeaves> {
        debug_assert!(at <= summary.bytes);
        for key in keys {
            summary.ids.insert(key.id());
        }

        // We search from right-to-left because usually insertions happen in increasing order by range.
        let mut offset = summary.bytes;
        for i in (0..self.extents.len()).rev() {
            offset -= self.extents[i].len();
            if at == offset {
                // There is an existing extent that exactly matches the insertion point.
                self.extents[i].keys.extend(keys.iter().map(|key| key.into_raw()));
                return None;
            }

            if offset < at {
                // Split the extent at the insertion point, the keys start the second part. At the
                // end of the leaf, the second part is empty.
                let rem = self.extents[i].length - (at - offset);
                self.extents[i].length -= rem;
                let extent = Extent::new(rem, keys.iter().copied());
                return match self.extents.try_insert(i + 1, extent) {
                    Ok(()) => None,
                    Err(err) => {
                        let mut entries =
                            self.extents.take().into_iter().collect::<SmallVec<_, N>>();
                        entries.insert(i + 1, err.element());
                        self.set_extents(summary, entries)
                    }
                };
            }
        }

        unreachable!("the first extent starts at offset 0")
    }

    /// The fewest extents a leaf that isn't the root can have, see `is_underfilled`.
    fn min_extents() -> usize {
        (N / 4).clamp(2, N.div_ceil(2))
    }

    /// Makes `entries` the leaf's extents. If they don't fit, they fill this leaf and the returned
    /// leaves that follow it in order, as appending marks in order leaves full leaves behind. The
    /// last leaf takes enough extents not to be underfilled, at least two, so a trailing empty
    /// extent is never alone in a leaf.
    fn set_extents(
        &mut self,
        summary: &mut Summary,
        entries: SmallVec<Extent, N>,
    ) -> Option<<Self as ReplaceableLeaf<ByteMetric>>::ExtraLeaves> {
        let leaves = entries.len().div_ceil(N).max(1);
        let last = match leaves {
            1 => entries.len(),
            _ => (entries.len() - (leaves - 1) * N).max(Self::min_extents().max(2)),
        };
        let mut rest = entries.len() - last;
        let mut entries = entries.into_iter();
        let mut chunks = (0..leaves).map(|i| {
            let len = if i + 1 == leaves { last } else { rest.min(N) };
            rest = rest.saturating_sub(len);
            Leaf::from(entries.by_ref().take(len).collect::<ArrayVec<_, N>>())
        });

        *self = chunks.next().expect("there is at least one leaf");
        *summary = self.summarize();
        let extra = chunks.collect::<SmallVec<_, 1>>();
        (!extra.is_empty()).then(|| extra.into_iter())
    }
}

impl<const N: usize> From<ArrayVec<Extent, N>> for Leaf<N> {
    #[inline]
    fn from(entries: ArrayVec<Extent, N>) -> Self {
        Self { extents: entries }
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
        Self::from(ArrayVec::try_from(slice.extents).unwrap())
    }
}

impl<const N: usize> ReplaceableLeaf<ByteMetric> for Leaf<N> {
    type Replacement<'a> = Replacement;

    type ExtraLeaves = smallvec::IntoIter<Leaf<N>, 1>;

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
        debug_assert!(end <= n, "end <= n ({end} <= {n})");

        match replace_with {
            Replacement::Gap(gap) => self.shift(summary, start..end, gap),
            Replacement::Keys(keys) => {
                assert_eq!(start, end);
                self.insert(summary, start, &keys)
            }
        }
    }

    #[inline]
    fn remove_up_to(&mut self, summary: &mut Self::Summary, up_to: ByteMetric) {
        assert!(self.replace(summary, ..up_to, Replacement::Gap(0)).is_none(), "how to handle");
    }
}

impl<const N: usize> BalancedLeaf for Leaf<N> {
    /// A leaf is underfilled with fewer than two extents, as only its first extent can be keyless,
    /// or fewer than a quarter of `N`, so leaves aren't rebalanced after every few deletes. Two
    /// leaves either fit in one or split into halves that aren't underfilled. An empty leaf is
    /// always underfilled: `crop` drops a leaf without bytes after balancing it, so one that holds
    /// keys must be merged first.
    #[inline]
    fn is_underfilled(&self, summary: &Self::Summary) -> bool {
        self.extents.len() < Self::min_extents() || summary.bytes == 0
    }

    fn balance_leaves(
        (left, left_summary): (&mut Self, &mut Self::Summary),
        (right, right_summary): (&mut Self, &mut Self::Summary),
    ) {
        if !left.is_underfilled(left_summary) && !right.is_underfilled(right_summary) {
            return;
        }

        let mut builder = ExtentBuilder::<N>::default();
        for extent in left.extents.take().iter().chain(&right.extents.take()) {
            builder.push_extent(extent);
        }

        // Split the rest so `right` gets the larger half, at least two extents, so some bytes.
        let entries = builder.finish();
        let mid = if entries.len() <= N { entries.len() } else { entries.len() / 2 };
        let mut entries = entries.into_iter();
        left.extents = entries.by_ref().take(mid).collect();
        right.extents = entries.collect();
        *left_summary = left.summarize();
        *right_summary = right.summarize();
    }
}

impl<const N: usize> Summarize for Leaf<N> {
    type Summary = Summary;

    #[inline]
    fn summarize(&self) -> Self::Summary {
        self.as_slice().summarize()
    }
}

impl<const N: usize> BaseMeasured for Leaf<N> {
    type BaseMetric = ByteMetric;
}

impl<const N: usize> AsSlice for Leaf<N> {
    type Slice<'a>
        = LeafSlice<'a>
    where
        Self: 'a;

    #[inline]
    fn as_slice(&self) -> Self::Slice<'_> {
        LeafSlice { extents: &self.extents }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct LeafSlice<'a> {
    pub(crate) extents: &'a [Extent],
}

impl fmt::Debug for LeafSlice<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.extents.fmt(f)
    }
}

impl Default for LeafSlice<'_> {
    #[inline]
    fn default() -> Self {
        Self { extents: &[] }
    }
}

impl LeafSlice<'_> {
    /// Return the item with the given `id` if it exists.
    /// The item `byte` is relative to the start of the leaf node.
    fn get_left(&self, id: u64) -> Option<usize> {
        let mut offset = 0;
        for extent in self.extents {
            if extent.keys.contains(Key::new(id, Flags::empty()).into_raw()) {
                // Fast path if the flags are empty.
                return Some(offset);
            }
            // Otherwise, scan. This may find an empty range mark's end key first, at the same offset.
            if extent.ids().any(|key_id| key_id == id) {
                return Some(offset);
            }

            offset += extent.len();
        }

        None
    }

    /// Return the item with the given `id` if it exists.
    /// The item `byte` is the distance to the end of the leaf node.
    fn get_right(&self, id: u64) -> Option<usize> {
        let mut offset = 0;

        for extent in self.extents.iter().rev() {
            if extent.keys.contains(Key::new(id, Flags::empty()).into_raw()) {
                // Fast path if the flags are empty.
                return Some(offset + extent.len());
            }
            // Otherwise, scan.
            // Technically we should scan from the right.
            for key in extent.keys() {
                if key.id() == id {
                    return Some(offset + extent.len());
                }
            }

            offset += extent.len();
        }

        None
    }
}

impl Summarize for LeafSlice<'_> {
    type Summary = Summary;

    #[inline]
    fn summarize(&self) -> Self::Summary {
        Summary {
            bytes: self.extents.iter().map(Extent::len).sum(),
            ids: FromIterator::from_iter(self.extents.iter().flat_map(Extent::ids)),
        }
    }
}
