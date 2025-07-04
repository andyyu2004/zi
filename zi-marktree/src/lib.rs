#![feature(
    array_chunks,
    iter_array_chunks,
    coroutines,
    iter_from_coroutine,
    debug_closure_helpers,
    iter_map_windows
)]

mod bitbag;
mod builder;

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Range, RangeBounds, Sub, SubAssign};
use std::{cmp, fmt, iter};

use arrayvec::ArrayVec;
use crop::tree::{
    Arc, AsSlice, BalancedLeaf, BaseMeasured, Metric, Node, ReplaceableLeaf, Summarize, Tree,
};
use smallvec::{SmallVec, smallvec};
use stdx::iter::ExactChain;
use stdx::range::RangeExt;
use tinyset::SetU64;

use self::bitbag::Bitbag;
pub use self::builder::MarkBuilder;
use self::extent_builder::ExtentBuilder;
use self::key::{Flags, Key};

pub trait MarkTreeId: Copy + Eq + From<u32> + Into<u32> + fmt::Debug + 'static {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bias {
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
/// This has a similar API to a `BTreeMap<Id, Range<usize>>` but is augmented with an efficient
/// `shift` operation to perform edits.
//
// This is implemented in a way that is basically a rope but instead of representing
// character data per byte, it stores a set of `MarkId`s.
//
// This is further optimized by compressing each byte into `Extent`s which represents a range containing a set of `MarkId`s.
//
// It is also extended to represent a range per mark not just an offset. This is done by inserting
// two entries for the same id representing the start and end of the range.
//
// Plenty of optimizations available. The implementation is fairly naive.
//  - avoid recreating bitmaps and arrays from scratch all the time
pub struct MarkTree<Id: MarkTreeId, const N: usize> {
    tree: Tree<ARITY, Leaf<N>>,
    _id: PhantomData<Id>,
}

impl<Id: MarkTreeId, const N: usize> fmt::Debug for MarkTree<Id, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.tree.fmt(f)
    }
}

impl<const N: usize, Id: MarkTreeId> MarkTree<Id, N> {
    /// Creates a new `MarkTree` appropriate for a text of length `n`.
    pub fn new(n: usize) -> Self {
        // assert!(n > 0, "MarkTree must have a non-zero length");
        let mut this = Self { tree: Tree::default(), _id: PhantomData };
        this.replace(0..0, Replacement::Gap(n));
        this
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tree.summary().bytes
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn get(&self, id: impl Into<Id>) -> Option<Range<usize>> {
        let id = id.into().into();
        let start = self.get_left(id)?;
        let end = self.get_right(id)?;
        Some(start..end)
    }

    fn get_left(&self, id: u32) -> Option<usize> {
        let (offset, leaf) = self.find_left_leaf(id)?;
        leaf.get_left(id).map(|byte| offset + byte)
    }

    fn get_right(&self, id: u32) -> Option<usize> {
        let (offset, leaf) = self.find_right_leaf(id)?;
        leaf.get_right(id).map(|byte| offset - byte)
    }

    /// Return the `(offset, leaf)` pair of the leftmost leaf that contains the given `id`.
    fn find_left_leaf(&self, id: u32) -> Option<(usize, &Leaf<N>)> {
        // Need to do a manual traversal to make use of the bitmaps.
        let mut node = self.tree.root().as_ref();
        if !node.summary().ids.contains(id) {
            return None;
        }

        let mut offset = 0;

        loop {
            debug_assert!(node.summary().ids.contains(id));
            match node {
                Node::Internal(inode) => {
                    node = inode
                        .children()
                        .iter()
                        .find(|child| {
                            let summary = child.summary();
                            if summary.ids.contains(id) {
                                true
                            } else {
                                offset += summary.bytes;
                                false
                            }
                        })?
                        .as_ref();
                }
                Node::Leaf(leaf) => return Some((offset, leaf.value())),
            }
        }
    }

    /// Return the `(end_offset, leaf)` pair of the rightmost leaf that contains the given `id`.
    fn find_right_leaf(&self, id: u32) -> Option<(usize, &Leaf<N>)> {
        let mut node = self.tree.root().as_ref();
        if !node.summary().ids.contains(id) {
            return None;
        }

        let mut offset = node.summary().bytes;

        loop {
            debug_assert!(node.summary().ids.contains(id));
            match node {
                Node::Internal(inode) => {
                    node = inode
                        .children()
                        .iter()
                        .rev()
                        .find(|child| {
                            let summary = child.summary();
                            if summary.ids.contains(id) {
                                true
                            } else {
                                offset -= summary.bytes;
                                false
                            }
                        })?
                        .as_ref();
                }
                Node::Leaf(leaf) => return Some((offset, leaf.value())),
            }
        }
    }

    pub fn shift(&mut self, range: impl RangeBounds<usize>, by: usize) {
        self.replace(range, Replacement::Gap(by));
        self.tree.assert_invariants();
    }

    /// Returns an iterator over the items whose start point is in the given range.
    pub fn range(
        &self,
        range: impl RangeBounds<usize>,
    ) -> impl Iterator<Item = (Range<usize>, Id)> + '_ {
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        let mut q = VecDeque::from([(0, self.tree.root().as_ref())]);

        iter::from_coroutine(
            #[coroutine]
            move || {
                while let Some((mut offset, node)) = q.pop_front() {
                    match node {
                        Node::Internal(inode) => {
                            for child in inode.children().iter() {
                                let summary = child.summary();
                                if (offset..offset + summary.bytes).intersects(&(start..end)) {
                                    // Child node has intersection with the relevant range,
                                    // continue exploring it.
                                    q.push_back((offset, child.as_ref()));
                                }

                                offset += summary.bytes;
                            }
                        }
                        Node::Leaf(leaf) => {
                            for extent in leaf.as_slice().extents {
                                if offset >= end {
                                    break;
                                }

                                if offset < start {
                                    offset += extent.len();
                                    continue;
                                }

                                for key in extent.keys() {
                                    let flags = key.flags();
                                    if flags.contains(Flags::END) {
                                        continue;
                                    }

                                    if flags.contains(Flags::RANGE) {
                                        let end = self
                                            .get_right(key.id())
                                            .expect("we should at least find the current key");
                                        yield (offset..end, key.id().into());
                                    } else {
                                        yield (offset..offset, key.id().into());
                                    }
                                }

                                offset += extent.len();
                            }
                        }
                    }
                }
            },
        )
    }

    /// Inserts an item based on its byte position.
    /// This does not affect `self.len()`.
    pub fn insert(&mut self, at: usize, id: Id) -> Inserter<'_, Id, N> {
        Inserter { tree: self, id, builder: MarkBuilder::new(at) }
    }

    pub fn drain(&mut self, range: impl RangeBounds<usize>) -> Drain<'_, Id, N>
    where
        Self: Sized,
    {
        let ids = self.range(range).map(|(_, id)| id).collect::<Vec<_>>().into_iter();
        Drain { tree: self, ids }
    }

    pub fn delete(&mut self, id: impl Into<Id>) -> Option<Range<usize>> {
        fn del<const N: usize>(
            node: &mut Arc<Node<ARITY, Leaf<N>>>,
            mut offset: usize,
            id: u32,
        ) -> usize {
            match Arc::make_mut(node) {
                Node::Internal(inode) => {
                    for i in 0..inode.children().len() {
                        let summary = inode.child(i).summary();
                        if summary.ids.contains(id) {
                            return inode.with_child_mut(i, |child| del(child, offset, id));
                        }
                        offset += summary.bytes;
                    }
                    unreachable!("bitmaps said it's here")
                }
                Node::Leaf(leaf) => {
                    debug_assert!(leaf.summary().ids.contains(id));
                    let leaf_offset =
                        leaf.value.delete(&mut leaf.summary, id).expect("bitmap said it's here");
                    offset + leaf_offset
                }
            }
        }

        let id = id.into().into();
        let root = self.tree.root_mut();
        if !root.summary().ids.contains(id) {
            return None;
        }

        let start = del(root, 0, id);

        if !root.summary().ids.contains(id) {
            return Some(start..start);
        }

        let range = start..del(root, 0, id);
        self.tree.assert_invariants();
        Some(range)
    }

    fn replace(&mut self, range: impl RangeBounds<usize>, replace_with: Replacement) {
        let initial_len = self.len();
        let k = replace_with.width();
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        self.tree.replace(ByteMetric(start)..ByteMetric(end), replace_with);
        debug_assert_eq!(self.len() + end, initial_len + start + k);
    }

    #[doc(hidden)]
    #[track_caller]
    pub fn assert_invariants(&self) {
        #[cfg(debug_assertions)]
        {
            // Ensure that the summaries are all correct by resummarizing the tree.
            #[allow(unused)]
            #[track_caller]
            fn summarize<const N: usize>(node: &Arc<Node<ARITY, Leaf<N>>>) -> Summary {
                let summary = match node.as_ref() {
                    Node::Internal(inode) => inode
                        .children()
                        .iter()
                        .fold(Summary::default(), |summary, child| summary + summarize(child)),
                    Node::Leaf(leaf) => leaf.as_slice().summarize(),
                };

                assert_eq!(node.summary(), &summary);
                summary
            }

            // Too slow, uncomment for debugging only.
            // summarize(&self.tree.root());
            // self.tree.assert_invariants();
            // for &id in &self.tree.summary().ids {
            //     self.get(id).expect("id in summary but not found");
            // }
        }
    }
}

/// A builder for inserting an item into a `MarkTree`, inserts on drop.
#[derive(Debug)]
pub struct Inserter<'a, Id: MarkTreeId, const N: usize> {
    tree: &'a mut MarkTree<Id, N>,
    id: Id,
    builder: MarkBuilder,
}

impl<Id: MarkTreeId, const N: usize> Inserter<'_, Id, N> {
    pub fn start_bias(mut self, bias: Bias) -> Self {
        self.builder = self.builder.start_bias(bias);
        self
    }

    pub fn end_bias(mut self, bias: Bias) -> Self {
        self.builder = self.builder.end_bias(bias);
        self
    }

    pub fn width(mut self, width: usize) -> Self {
        self.builder = self.builder.width(width);
        self
    }
}

impl<Id: MarkTreeId, const N: usize> Drop for Inserter<'_, Id, N> {
    fn drop(&mut self) {
        let id = self.id.into();
        let at = self.builder.at;
        let n = self.tree.len();

        if self.tree.tree.summary().ids.contains(id) {
            self.tree.delete(id).unwrap();
        }

        assert!(
            at + self.builder.width <= self.tree.len(),
            "range {at}..{} out of bounds of marktree of length {}",
            self.builder.at + self.builder.width,
            self.tree.len(),
        );

        self.tree.replace(at..at, Replacement::Key(Key::new(id, self.builder.start_flags)));
        assert_eq!(self.tree.len(), n, "first insertion should not change the length of the tree");

        if self.builder.start_flags.contains(Flags::RANGE) {
            assert!(self.builder.end_flags.contains(Flags::RANGE | Flags::END));
            let at = at + self.builder.width;
            self.tree.replace(at..at, Replacement::Key(Key::new(id, self.builder.end_flags)));
            assert_eq!(
                self.tree.len(),
                n,
                "second insertion should not change the length of the tree"
            );
        }

        self.tree.assert_invariants();
    }
}

#[derive(Debug)]
enum Replacement {
    Gap(usize),
    // Invariant, `Key` can only be used as a replacement if the range is empty.
    // The replacement range is `byte..byte`
    Key(Key),
}

impl Replacement {
    fn width(&self) -> usize {
        match self {
            Replacement::Gap(n) => *n,
            Replacement::Key(_) => 0,
        }
    }
}

pub struct Drain<'a, Id: MarkTreeId, const N: usize> {
    tree: &'a mut MarkTree<Id, N>,
    ids: std::vec::IntoIter<Id>,
}

impl<Id: MarkTreeId, const N: usize> Drop for Drain<'_, Id, N> {
    fn drop(&mut self) {
        for id in self.ids.by_ref() {
            self.tree.delete(id).unwrap();
        }
    }
}

impl<Id: MarkTreeId, const N: usize> Iterator for Drain<'_, Id, N> {
    type Item = (Range<usize>, Id);

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

#[derive(Clone, PartialEq, Eq)]
struct Extent {
    length: usize,
    /// The ids contained within this range.
    /// All their positions is considered to be the start of the extent.
    keys: SetU64,
}

impl fmt::Debug for Extent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("")
            .field(&self.length)
            .field_with(|f| f.debug_set().entries(self.keys()).finish())
            .finish()
    }
}

impl Extent {
    #[track_caller]
    fn new(length: usize, keys: impl IntoIterator<Item = Key>) -> Self {
        Self { length, keys: keys.into_iter().map(Key::into_raw).collect() }
    }

    fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.keys.iter().map(|key| key as u32)
    }

    fn keys(&self) -> impl Iterator<Item = Key> + '_ {
        self.keys.iter().map(Key::from_raw)
    }

    #[inline]
    fn len(&self) -> usize {
        self.length
    }
}

#[derive(Clone)]
struct Leaf<const N: usize> {
    extents: ArrayVec<Extent, N>,
}

impl<const N: usize> fmt::Debug for Leaf<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.extents.iter()).finish()
    }
}

impl<const N: usize> Leaf<N> {
    #[inline]
    fn get_left(&self, id: u32) -> Option<usize> {
        self.as_slice().get_left(id)
    }

    #[inline]
    fn get_right(&self, id: u32) -> Option<usize> {
        self.as_slice().get_right(id)
    }

    fn delete(&mut self, summary: &mut Summary, id: u32) -> Option<usize> {
        let mut offset = 0;

        for extent in &mut self.extents {
            if extent.keys.remove(id as u64) {
                // Fast path if the flags are empty.
                assert!(summary.ids.remove(id).is_some());
                return Some(offset);
            } else {
                // Otherwise, we have to linearly scan the map to find the id since the keys contain the flags too.
                let mut iter = extent.keys.iter();
                while let Some(key) = iter.next() {
                    let key = Key::from_raw(key);
                    if key.id() == id {
                        drop(iter);
                        assert!(extent.keys.remove(key.into_raw()));
                        assert!(summary.ids.remove(id).is_some());
                        return Some(offset);
                    }
                }
            }

            offset += extent.len();
        }

        None
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
                builder.push_extent(extent);
                offset += k;

                continue;
            }

            keys = keys | &extent.keys;

            match start.cmp(&offset) {
                cmp::Ordering::Greater => {
                    // The offset is before the start of the replacement range.
                    // Copy the chunk of the extent that precedes the replacement range.
                    builder.push_raw(start - offset, keys.drain())
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
            builder.push_raw(gap, keys)
        } else if !keys.is_empty() {
            builder.push_raw(0, keys);
        }

        let entries = builder.finish();
        let mut chunks = entries.array_chunks::<N>();
        let (chunk, used_remainder) = match chunks.next() {
            Some(chunk) => (ArrayVec::from(chunk.clone()), false),
            None => (
                ArrayVec::try_from(chunks.remainder()).expect("remainder can't be too large"),
                true,
            ),
        };

        self.extents = chunk;
        assert!(!self.extents.is_empty());

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

mod extent_builder {
    use super::*;

    #[derive(Debug)]
    pub(super) struct ExtentBuilder<const N: usize> {
        extents: SmallVec<Extent, N>,
    }

    impl<const N: usize> Default for ExtentBuilder<N> {
        fn default() -> Self {
            Self { extents: SmallVec::new() }
        }
    }

    impl<const N: usize> ExtentBuilder<N> {
        pub fn finish(self) -> SmallVec<Extent, N> {
            self.extents
        }

        #[track_caller]
        pub fn push(&mut self, length: usize, keys: impl IntoIterator<Item = Key>) {
            let mut keys = keys.into_iter().peekable();
            match self.extents.last_mut() {
                Some(last) if last.length == 0 || keys.peek().is_none() => {
                    // Merge entries if possible.
                    last.length += length;
                    last.keys.extend(keys.map(Key::into_raw));
                }
                _ => self.extents.push(Extent::new(length, keys)),
            }
        }

        #[track_caller]
        pub fn push_raw(&mut self, length: usize, keys: impl IntoIterator<Item = u64>) {
            self.push(length, keys.into_iter().map(Key::from_raw));
        }

        pub fn push_gap(&mut self, gap: usize) {
            if gap > 0 {
                self.push(gap, []);
            }
        }

        #[track_caller]
        pub fn push_extent(&mut self, extent: Extent) {
            self.push_raw(extent.len(), extent.keys().map(Key::into_raw));
        }
    }
}

mod key {
    use std::{fmt, mem};

    bitflags::bitflags! {
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub struct Flags: u32 {
            const BIAS_LEFT = 1 << 0;
            // If the key is part of a range pair.
            const RANGE = 1 << 1;
            /// The end of a range pair.
            const END = 1 << 2;
        }
    }

    impl fmt::Debug for Flags {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fmt(f)
        }
    }

    /// Key encodes the 32-bit id and 32-bit flags.
    // We don't nearly need 32-bits of flag space, but we're keeping the id small to allow for optimizations.
    #[derive(Clone, Copy)]
    pub(super) struct Key(u64);

    impl fmt::Debug for Key {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_tuple("").field(&self.id()).field(&self.flags()).finish()
        }
    }

    impl Key {
        const FLAG_BITS: usize = 32;
        const ID_BITS: usize = mem::size_of::<u64>() * 8 - Self::FLAG_BITS;

        pub fn new(id: u32, flag: Flags) -> Self {
            Self(id as u64 | ((flag.bits() as u64) << 32))
        }

        #[inline]
        pub fn id(self) -> u32 {
            self.0 as u32
        }

        #[inline]
        pub fn flags(self) -> Flags {
            Flags::from_bits((self.0 >> Self::ID_BITS) as u32).unwrap()
        }

        #[inline]
        pub fn into_raw(self) -> u64 {
            self.0
        }

        #[inline]
        pub fn from_raw(id: u64) -> Self {
            Self(id)
        }
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
struct LeafSlice<'a> {
    extents: &'a [Extent],
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
    fn get_left(&self, id: u32) -> Option<usize> {
        let mut offset = 0;
        for extent in self.extents {
            if extent.keys.contains(id as u64) {
                // Fast path if the flags are empty.
                return Some(offset);
            } else {
                // Otherwise, scan
                for key in extent.keys() {
                    if key.id() == id {
                        assert!(!key.flags().contains(Flags::END));
                        return Some(offset);
                    }
                }
            }

            offset += extent.len();
        }

        None
    }

    /// Return the item with the given `id` if it exists.
    /// The item `byte` is the distance to the end of the leaf node.
    fn get_right(&self, id: u32) -> Option<usize> {
        let mut offset = 0;

        for extent in self.extents.iter().rev() {
            if extent.keys.contains(id as u64) {
                // Fast path if the flags are empty.
                return Some(offset + extent.len());
            } else {
                // Otherwise, scan.
                // Technically we should scan from the right.
                for key in extent.keys() {
                    if key.id() == id {
                        return Some(offset + extent.len());
                    }
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
            bytes: self.extents.iter().map(|extent| extent.len()).sum(),
            ids: FromIterator::from_iter(self.extents.iter().flat_map(|extent| extent.ids())),
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

impl AddAssign<&Extent> for Summary {
    #[inline]
    fn add_assign(&mut self, rhs: &Extent) {
        self.bytes += rhs.len();
        for key in rhs.keys() {
            // TODO probably is a faster way to do this
            self.ids.insert(key.id());
        }
    }
}

impl SubAssign<&Extent> for Summary {
    #[inline]
    fn sub_assign(&mut self, rhs: &Extent) {
        self.bytes -= rhs.len();
        for key in rhs.keys() {
            // TODO probably is a faster way to do this
            assert!(self.ids.remove(key.id()).is_some());
        }
    }
}

impl AddAssign<Self> for Summary {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self += &rhs;
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

#[derive(Default, Clone, PartialEq)]
struct Summary {
    /// This needs to be a `bag` not a `set` otherwise the `Sub` operation and `Add` operation will
    /// not be inverses of each other and `crop` assumptions break.
    ids: Bitbag,
    bytes: usize,
}

impl fmt::Debug for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("")
            .field(&self.bytes)
            .field_with(|f| f.debug_set().entries(self.ids.iter()).finish())
            .finish()
    }
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

    (start, end)
}
