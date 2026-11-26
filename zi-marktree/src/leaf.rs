use std::cmp;
use std::fmt;
use std::ops::{Range, RangeBounds};

use arrayvec::ArrayVec;
use crop::tree::{AsSlice, BalancedLeaf, BaseMeasured, ReplaceableLeaf, Summarize};
use smallvec::{SmallVec, smallvec};
use stdx::iter::ExactChain;
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

    pub(crate) fn remove(&mut self, summary: &mut Summary, ids: &SetU64) {
        let mut builder = ExtentBuilder::<N>::default();
        for extent in self.extents.take() {
            builder.push(extent.len(), extent.keys().filter(|key| !ids.contains(key.id())));
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

        let entries = builder.finish();
        let (chunks, remainder) = entries.as_chunks::<N>();
        let mut chunks = chunks.iter();
        let (chunk, used_remainder) = match chunks.next() {
            Some(chunk) => (ArrayVec::from(chunk.clone()), false),
            None => (ArrayVec::try_from(remainder).expect("remainder can't be too large"), true),
        };

        self.extents = chunk;
        assert!(!self.extents.is_empty());

        *summary = self.summarize();

        if chunks.len() == 0 && (used_remainder || remainder.is_empty()) {
            return None;
        }

        let rem = if remainder.is_empty() {
            None
        } else {
            Some(ArrayVec::try_from(remainder).expect("remainder can't be too large"))
        };

        Some(
            chunks
                .cloned()
                .map(ArrayVec::from)
                .exact_chain(rem)
                .map(Leaf::from)
                .collect::<SmallVec<_, 1>>()
                .into_iter(),
        )
    }

    fn insert(
        &mut self,
        summary: &mut Summary,
        at: usize,
        key: Key,
    ) -> Option<<Self as ReplaceableLeaf<ByteMetric>>::ExtraLeaves> {
        debug_assert!(at <= summary.bytes);
        if at == summary.bytes {
            // Trying to insert at the end. Just propagate the new key up.
            let array = ArrayVec::<_, N>::from_iter([Extent::new(0, [key])]);
            return Some(smallvec![Leaf::from(array)].into_iter());
        }

        // We search from right-to-left because usually insertions happen in increasing order by range.
        let mut offset = summary.bytes;
        for i in (0..self.extents.len()).rev() {
            offset -= self.extents[i].len();
            if at == offset {
                // There is an existing extent that exactly matches the insertion point.
                // Just add the new key to the extent and return.
                self.extents[i].keys.insert(key.into_raw());
                summary.ids.insert(key.id());
                break;
            }

            if offset < at {
                // The extent covers the insertion point.
                // Split the extent into two and add the key to the second extent.
                let rem = self.extents[i].length - (at - offset);
                self.extents[i].length -= rem;

                match self.extents.try_insert(i + 1, Extent::new(rem, [key])) {
                    Ok(()) => {
                        summary.ids.insert(key.id());
                    }
                    Err(err) => {
                        if i + 1 == N {
                            summary.bytes -= rem;
                            // Trying to insert at end, propagate the new key up.
                            let array = ArrayVec::<_, N>::from_iter([err.element()]);
                            return Some(smallvec![Leaf::from(array)].into_iter());
                        }

                        // If we can't fit, drain the remainder of the keys and propagate them up.
                        let drained = self
                            .extents
                            .drain(i + 1..)
                            .inspect(|extent| *summary -= extent)
                            .collect::<ArrayVec<_, N>>();

                        let extent = err.element();
                        for key in extent.keys() {
                            summary.ids.insert(key.id());
                        }
                        self.extents.insert(i + 1, extent);

                        return Some(smallvec![Leaf::from(drained)].into_iter());
                    }
                }
                break;
            }
        }

        None
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
            Replacement::Key(key) => {
                assert_eq!(start, end);
                self.insert(summary, start, key)
            }
        }
    }

    #[inline]
    fn remove_up_to(&mut self, summary: &mut Self::Summary, up_to: ByteMetric) {
        assert!(self.replace(summary, ..up_to, Replacement::Gap(0)).is_none(), "how to handle");
    }
}

impl<const N: usize> BalancedLeaf for Leaf<N> {
    #[inline]
    fn is_underfilled(&self, _summary: &Self::Summary) -> bool {
        false
    }

    #[inline]
    fn balance_leaves(
        _left: (&mut Self, &mut Self::Summary),
        _right: (&mut Self, &mut Self::Summary),
    ) {
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
            // Otherwise, scan
            for key in extent.keys() {
                if key.id() == id {
                    assert!(!key.flags().contains(Flags::END));
                    return Some(offset);
                }
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
