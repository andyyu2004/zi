#![feature(array_chunks, coroutines, iter_from_coroutine, debug_closure_helpers)]

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::ops::{Add, AddAssign, Range, RangeBounds, Sub, SubAssign};
use std::{cmp, fmt, iter};

use arrayvec::ArrayVec;
use crop::tree::{
    Arc, AsSlice, BalancedLeaf, BaseMeasured, Metric, Node, ReplaceableLeaf, Summarize, Tree,
};
use smallvec::{smallvec, SmallVec};
use stdx::iter::ExactChain;
use tinyset::SetU64;

use self::bitmap::Bitmap48;
use self::key::{Flags, Key};

pub trait MarkTreeId: Copy + Eq + From<u64> + Into<u64> + fmt::Debug + 'static {}

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
/// This can be edited efficiently (logarithmic time) with the `shift` operation.
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
        assert!(n > 0, "MarkTree must have a non-zero length");
        let mut this = Self { tree: Tree::default(), _id: PhantomData };
        this.replace(0..0, Replacement::Gap(n));
        this
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tree.summary().bytes
    }

    #[inline]
    pub fn get(&self, id: impl Into<Id>) -> Option<Range<usize>> {
        let id = id.into().into();
        let start = self.get_left(id)?;
        let end = self.get_right(id)?;
        Some(start..end)
    }

    fn get_left(&self, id: impl Into<Id>) -> Option<usize> {
        let id = id.into().into();
        let (offset, leaf) = self.find_left_leaf(id)?;
        leaf.get_left(id).map(|byte| offset + byte)
    }

    fn get_right(&self, id: impl Into<Id>) -> Option<usize> {
        let id = id.into().into();
        let (offset, leaf) = self.find_right_leaf(id)?;
        leaf.get_right(id).map(|byte| offset - byte)
    }

    /// Return the `(offset, leaf)` pair of the leftmost leaf that contains the given `id`.
    fn find_left_leaf(&self, id: impl Into<Id>) -> Option<(usize, &Leaf<N>)> {
        // Need to do a manual traversal to make use of the bitmaps.
        let mut node = self.tree.root().as_ref();
        let id = id.into().into();
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
    fn find_right_leaf(&self, id: impl Into<Id>) -> Option<(usize, &Leaf<N>)> {
        let mut node = self.tree.root().as_ref();
        let id = id.into().into();
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
        let initial_len = self.len();
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        self.tree.replace(ByteMetric(start)..ByteMetric(end), Replacement::Gap(by));
        debug_assert_eq!(self.len() + end, initial_len + by + start);
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
                                // TODO something like the following logic to avoid adding
                                // unnecessary nodes to the queue
                                // let child_range = (offset..offset + summary.bytes);
                                // if !child_range.contains(start..end) {
                                //     continue;
                                // }

                                q.push_back((offset, child.as_ref()));
                                offset += summary.bytes;
                                if offset >= end {
                                    break;
                                }
                            }
                        }
                        Node::Leaf(leaf) => {
                            for entry in leaf.as_slice().entries {
                                if offset >= end {
                                    break;
                                }

                                if offset < start {
                                    offset += entry.len();
                                    continue;
                                }

                                for key in entry.keys() {
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

                                offset += entry.len();
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
        Inserter {
            tree: self,
            id,
            at,
            width: 0,
            start_flags: Flags::empty(),
            end_flags: Flags::END,
        }
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

        Some(start..del(root, 0, id))
    }

    fn replace(&mut self, range: impl RangeBounds<usize>, replace_with: Replacement) {
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        self.tree.replace(ByteMetric(start)..ByteMetric(end), replace_with);
    }

    #[doc(hidden)]
    pub fn assert_invariants(&self) {
        #[cfg(debug_assertions)]
        {
            // Ensure that the summaries are all correct by resummarizing the tree.
            #[allow(unused)]
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
            summarize(&self.tree.root());
            self.tree.assert_invariants();
        }
    }
}

/// A builder for inserting an item into a `MarkTree`, inserts on drop.
#[derive(Debug)]
pub struct Inserter<'a, Id: MarkTreeId, const N: usize> {
    tree: &'a mut MarkTree<Id, N>,
    id: Id,
    at: usize,
    width: usize,
    start_flags: Flags,
    end_flags: Flags,
}

impl<'a, Id: MarkTreeId, const N: usize> Inserter<'a, Id, N> {
    pub fn start_bias(mut self, bias: Bias) -> Self {
        match bias {
            Bias::Left => self.start_flags.insert(Flags::BIAS_LEFT),
            Bias::Right => self.start_flags.remove(Flags::BIAS_LEFT),
        }
        self
    }

    pub fn end_bias(mut self, bias: Bias) -> Self {
        match bias {
            Bias::Left => self.end_flags.insert(Flags::BIAS_LEFT),
            Bias::Right => self.end_flags.remove(Flags::BIAS_LEFT),
        }
        self
    }

    pub fn width(mut self, width: usize) -> Self {
        if width > 0 {
            self.start_flags.insert(Flags::RANGE);
            self.end_flags.insert(Flags::RANGE);
        } else {
            self.start_flags.remove(Flags::RANGE);
            self.end_flags.remove(Flags::RANGE);
        }
        self.width = width;
        self
    }
}

impl<'a, Id: MarkTreeId, const N: usize> Drop for Inserter<'a, Id, N> {
    fn drop(&mut self) {
        let id = self.id.into();
        let at = self.at;
        let n = self.tree.len();

        // Check upper 16 bits are clear
        assert_eq!(id >> 48, 0, "upper 16 bits of id must be unused");

        if self.tree.tree.summary().ids.contains(id) {
            todo!("MarkTree insertion of existing id")
        }

        assert!(
            at + self.width < self.tree.len(),
            "range {at}..={} out of bounds of marktree of length {}",
            self.at + self.width,
            self.tree.len(),
        );

        self.tree.replace(at..=at, Replacement::Key(Key::new(id, self.start_flags)));
        assert_eq!(self.tree.len(), n, "first insertion should not change the length of the tree");
        self.tree.assert_invariants();

        if self.start_flags.contains(Flags::RANGE) {
            assert!(self.end_flags.contains(Flags::RANGE | Flags::END));
            let at = at + self.width;
            self.tree.replace(at..=at, Replacement::Key(Key::new(id, self.end_flags)));
            assert_eq!(
                self.tree.len(),
                n,
                "second insertion should not change the length of the tree"
            );
            self.tree.assert_invariants();
        }
    }
}

enum Replacement {
    Gap(usize),
    // Invariant, `Key` can only be used as a replacement if the range is empty. The replacement range is `byte..=byte`
    Key(Key),
}

pub struct Drain<'a, Id: MarkTreeId, const N: usize> {
    tree: &'a mut MarkTree<Id, N>,
    ids: std::vec::IntoIter<Id>,
}

impl<'a, Id: MarkTreeId, const N: usize> Drop for Drain<'a, Id, N> {
    fn drop(&mut self) {
        for id in self.ids.by_ref() {
            self.tree.delete(id).unwrap();
        }
    }
}

impl<'a, Id: MarkTreeId, const N: usize> Iterator for Drain<'a, Id, N> {
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

// NOTE: It's important to have a structure such that every leaf entry has a non-zero length.
// Otherwise, a zero-length entry could take up arbitrarily many slots in the tree which breaks assumptions by the tree impl.
//
// The current implementation naive and each entry represents a single byte.
// It would be better to have a more sophisticated implementation that can represent multiple bytes in a single entry (i.e. extents/ranges).
#[derive(Clone, PartialEq, Eq)]
struct Extent {
    length: NonZeroUsize,
    /// The ids contained within this range.
    /// All their positions is considered to be the start of the entry.
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
    fn new(length: usize, ids: impl IntoIterator<Item = Key>) -> Self {
        Self {
            length: NonZeroUsize::new(length).expect("leaf entry length must be > 0"),
            keys: ids.into_iter().map(Key::into_raw).collect(),
        }
    }

    fn ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.keys().map(Key::id)
    }

    fn keys(&self) -> impl Iterator<Item = Key> + '_ {
        self.keys.iter().map(Key::from_raw)
    }

    #[inline]
    fn len(&self) -> usize {
        self.length.get()
    }
}

#[derive(Clone)]
struct Leaf<const N: usize> {
    entries: ArrayVec<Extent, N>,
}

impl<const N: usize> fmt::Debug for Leaf<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.entries.iter()).finish()
    }
}

impl<const N: usize> Leaf<N> {
    #[inline]
    fn get_left(&self, id: u64) -> Option<usize> {
        self.as_slice().get_left(id)
    }

    #[inline]
    fn get_right(&self, id: u64) -> Option<usize> {
        self.as_slice().get_right(id)
    }

    fn delete(&mut self, summary: &mut Summary, id: u64) -> Option<usize> {
        let mut offset = 0;

        for entry in &mut self.entries {
            if entry.keys.remove(id) {
                // Fast path if the flags are empty.
                assert!(summary.ids.remove(id));
                return Some(offset);
            } else {
                // Otherwise, we have to linearly scan the map to find the id since the keys contain the flags too.
                let mut iter = entry.keys.iter();
                while let Some(key) = iter.next() {
                    let key = Key::from_raw(key);
                    if key.id() == id {
                        drop(iter);
                        assert!(entry.keys.remove(key.into_raw()));

                        if key.flags().contains(Flags::RANGE) {
                            // Just removing the id isn't correct since it's pair may still exist.
                            // Just resummarize the leaf for simplicity.
                            *summary = self.summarize();
                        } else {
                            assert!(summary.ids.remove(id));
                        }
                        return Some(offset);
                    }
                }
            }

            offset += entry.len();
        }

        None
    }
}

impl<const N: usize> From<ArrayVec<Extent, N>> for Leaf<N> {
    #[inline]
    fn from(entries: ArrayVec<Extent, N>) -> Self {
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

mod builder {
    use super::*;

    #[derive(Debug)]
    pub(super) struct EntryBuilder<const N: usize> {
        entries: SmallVec<Extent, N>,
    }

    impl<const N: usize> Default for EntryBuilder<N> {
        fn default() -> Self {
            Self { entries: SmallVec::new() }
        }
    }

    impl<const N: usize> EntryBuilder<N> {
        pub fn entries(&self) -> &[Extent] {
            &self.entries
        }

        pub fn finish(self) -> SmallVec<Extent, N> {
            self.entries
        }

        #[track_caller]
        pub fn push(&mut self, length: usize, keys: impl IntoIterator<Item = Key>) {
            let mut keys = keys.into_iter().peekable();
            match self.entries.last_mut() {
                Some(last) if keys.peek().is_none() => {
                    // Merge entries if possible.
                    last.length = last.length.checked_add(length).unwrap();
                }
                _ => self.entries.push(Extent::new(length, keys)),
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
        pub fn push_entry(&mut self, entry: Extent) {
            self.push_raw(entry.len(), entry.keys().map(Key::into_raw));
        }
    }
}

mod key {
    use std::{fmt, mem};

    bitflags::bitflags! {
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub struct Flags: u16 {
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

    /// Key encodes the 48-bit id and 16-bit flags.
    #[derive(Clone, Copy)]
    pub(super) struct Key(u64);

    impl fmt::Debug for Key {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_tuple("").field(&self.id()).field(&self.flags()).finish()
        }
    }

    impl Key {
        const FLAG_BITS: usize = 16;
        const ID_BITS: usize = mem::size_of::<u64>() * 8 - Self::FLAG_BITS;

        pub fn new(id: u64, flag: Flags) -> Self {
            Self(id | ((flag.bits() as u64) << 48))
        }

        #[inline]
        pub fn id(self) -> u64 {
            self.0 << Self::FLAG_BITS >> Self::FLAG_BITS
        }

        #[inline]
        pub fn flags(self) -> Flags {
            Flags::from_bits((self.0 >> Self::ID_BITS) as u16).unwrap()
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
        assert!(end <= n, "end <= n ({end} <= {n})");

        let mut builder = builder::EntryBuilder::<N>::default();
        let mut keys = SetU64::default();
        match replace_with {
            Replacement::Gap(gap) => {
                let mut gap = Some(gap);

                let mut offset = 0;
                for entry in self.entries.take() {
                    let entry_end = offset + entry.len();
                    let k = entry.len();

                    if entry_end < start || offset > end {
                        // If the entry range does not intersect the replacement range just copy
                        builder.push_entry(entry);
                        offset += k;

                        continue;
                    }

                    keys = keys | &entry.keys;

                    match start.cmp(&offset) {
                        cmp::Ordering::Greater => {
                            // The offset is before the start of the replacement range.
                            // Copy the chunk of the entry that precedes the replacement range.
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

                    if entry_end > end {
                        if let Some(gap) = gap.take() {
                            builder.push_gap(gap);
                        }
                        // If the entry extends beyond the replacement,
                        // push the remaining (right-biased only?) keys after.
                        builder.push_raw(entry_end - end, keys.drain());
                    }

                    offset += k;
                }

                if let Some(gap) = gap {
                    builder.push_gap(gap)
                };
            }
            Replacement::Key(key) => {
                if start == end {
                    // We usually expect `start + 1 = end`.
                    // However, this can occur if we're appending at the end of a leaf.
                    debug_assert_eq!(end, n);
                    debug_assert_eq!(
                        *summary,
                        self.summarize(),
                        "this node should remain unchanged"
                    );
                    let extent = [Extent::new(1, iter::once(key))];
                    return Some(smallvec![Leaf::from(ArrayVec::from_iter(extent))].into_iter());
                }

                // We're assuming that we're replacing a range of one byte to avoid the zero-length entry issue.
                assert_eq!(start + 1, end);

                let mut offset = 0;
                for entry in self.entries.take() {
                    let entry_end = offset + entry.len();
                    let k = entry.len();

                    if entry_end <= start || offset >= end {
                        // If the entry range does not intersect the replacement range just copy
                        builder.push_entry(entry);
                        offset += k;
                        continue;
                    }

                    // Therefore: offset < end && start < entry_end

                    if start - offset > 0 {
                        // The current entry extends beyond the start of the replacement range.
                        // Add the chunk of the entry that precedes the replacement range.
                        builder.push_raw(start - offset, entry.keys);
                        // Push a new entry for the key with length 1.
                        builder.push(1, [key]);
                    } else {
                        // Otherwise, they can be merged
                        let mut keys = entry.keys;
                        assert!(keys.insert(key.into_raw()));
                        builder.push_raw(start - offset + 1, keys);
                    }

                    if entry_end > end {
                        builder.push_gap(entry_end - end);
                    }

                    offset += k;
                }

                debug_assert_eq!(
                    builder.entries().iter().map(|entry| entry.len()).sum::<usize>(),
                    n,
                    "adding an item should not change the total length of the leaf"
                );
            }
        };

        let entries = builder.finish();
        let mut chunks = entries.array_chunks::<N>();
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
                .collect::<SmallVec<_, 1>>()
                .into_iter(),
        )
    }

    #[inline]
    fn remove_up_to(&mut self, summary: &mut Self::Summary, up_to: ByteMetric) {
        self.replace(summary, ..up_to, Replacement::Gap(0));
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
    type Slice<'a> = LeafSlice<'a > where Self: 'a;

    #[inline]
    fn as_slice(&self) -> Self::Slice<'_> {
        LeafSlice { entries: &self.entries }
    }
}

#[derive(Debug, Clone, Copy)]
struct LeafSlice<'a> {
    entries: &'a [Extent],
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
    fn get_left(&self, id: u64) -> Option<usize> {
        let mut offset = 0;
        for entry in self.entries {
            if entry.keys.contains(id) {
                // Fast path if the flags are empty.
                return Some(offset);
            } else {
                // Otherwise, scan
                for key in entry.keys() {
                    if key.id() == id {
                        assert!(!key.flags().contains(Flags::END));
                        return Some(offset);
                    }
                }
            }

            offset += entry.len();
        }

        None
    }

    /// Return the item with the given `id` if it exists.
    /// The item `byte` is the distance to the end of the leaf node.
    fn get_right(&self, id: u64) -> Option<usize> {
        let mut offset = 0;

        for entry in self.entries.iter().rev() {
            if entry.keys.contains(id) {
                // Fast path if the flags are empty.
                return Some(offset + entry.len());
            } else {
                // Otherwise, scan
                for key in entry.keys() {
                    if key.id() == id {
                        assert!(key.flags().contains(Flags::END));
                        return Some(offset + entry.len());
                    }
                }
            }

            offset += entry.len();
        }

        None
    }
}

impl<'a> Summarize for LeafSlice<'a> {
    type Summary = Summary;

    #[inline]
    fn summarize(&self) -> Self::Summary {
        Summary {
            bytes: self.entries.iter().map(|entry| entry.len()).sum(),
            ids: Bitmap48::from_iter(self.entries.iter().flat_map(|entry| entry.ids())),
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
        *self += rhs.clone();
    }
}

impl SubAssign<&Self> for Summary {
    #[inline]
    fn sub_assign(&mut self, rhs: &Self) {
        *self -= rhs.clone();
    }
}

#[derive(Default, Clone, PartialEq)]
struct Summary {
    ids: Bitmap48,
    bytes: usize,
}

impl fmt::Debug for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("").field(&self.bytes).field(&self.ids).finish()
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

mod bitmap {
    use std::collections::btree_map::Entry;
    use std::collections::BTreeMap;
    use std::fmt;
    use std::ops::{BitOrAssign, SubAssign};

    use roaring::RoaringBitmap;

    /// A bitmap with 48-bit values.
    /// Basically the same as `roaring::RoaringTreemap` but optimized for 48 bit values.
    /// It panics if the upper 16 bits are set which is useful to catch errors.
    #[derive(Default, Clone, PartialEq)]
    pub struct Bitmap48(BTreeMap<u16, RoaringBitmap>);

    impl fmt::Debug for Bitmap48 {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_set().entries(self.iter()).finish()
        }
    }

    impl Bitmap48 {
        pub fn new() -> Self {
            Self(BTreeMap::new())
        }

        pub fn insert(&mut self, value: u64) -> bool {
            let (hi, lo) = split(value);
            self.0.entry(hi).or_default().insert(lo)
        }

        pub fn contains(&self, value: u64) -> bool {
            let (hi, lo) = split(value);
            self.0.get(&hi).map_or(false, |map| map.contains(lo))
        }

        pub fn iter(&self) -> impl Iterator<Item = u64> + '_ {
            self.0.iter().flat_map(|(&hi, map)| map.iter().map(move |lo| combine(hi, lo)))
        }

        pub fn remove(&mut self, value: u64) -> bool {
            let (hi, lo) = split(value);
            match self.0.entry(hi) {
                Entry::Vacant(_) => false,
                Entry::Occupied(mut ent) => {
                    if ent.get_mut().remove(lo) {
                        if ent.get().is_empty() {
                            ent.remove();
                        }
                        true
                    } else {
                        false
                    }
                }
            }
        }
    }

    impl BitOrAssign<Bitmap48> for Bitmap48 {
        fn bitor_assign(&mut self, rhs: Bitmap48) {
            for (key, other_rb) in rhs.0 {
                match self.0.entry(key) {
                    Entry::Vacant(ent) => drop(ent.insert(other_rb)),
                    Entry::Occupied(mut ent) => *ent.get_mut() |= other_rb,
                }
            }
        }
    }

    impl BitOrAssign<&Self> for Bitmap48 {
        #[inline]
        fn bitor_assign(&mut self, rhs: &Self) {
            *self |= rhs.clone();
        }
    }

    impl SubAssign<&Self> for Bitmap48 {
        #[inline]
        fn sub_assign(&mut self, rhs: &Self) {
            *self -= rhs.clone();
        }
    }

    impl SubAssign<Self> for Bitmap48 {
        fn sub_assign(&mut self, rhs: Self) {
            for (key, rhs_rb) in rhs.0 {
                match self.0.entry(key) {
                    Entry::Vacant(_entry) => (),
                    Entry::Occupied(mut entry) => {
                        *entry.get_mut() -= rhs_rb;
                        if entry.get().is_empty() {
                            entry.remove_entry();
                        }
                    }
                }
            }
        }
    }

    impl FromIterator<u64> for Bitmap48 {
        fn from_iter<I: IntoIterator<Item = u64>>(iter: I) -> Self {
            let mut bitmap = Self::new();
            iter.into_iter().for_each(|id| {
                bitmap.insert(id);
            });
            bitmap
        }
    }

    #[inline]
    fn split(value: u64) -> (u16, u32) {
        assert_eq!(value >> 48, 0, "upper 16 bits of value must be unused");
        ((value >> 32) as u16, value as u32)
    }

    fn combine(hi: u16, lo: u32) -> u64 {
        ((hi as u64) << 32) | lo as u64
    }
}
