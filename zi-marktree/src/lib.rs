mod bitbag;
mod builder;
mod extent;
mod key;
mod leaf;
mod summary;

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::ops::{Add, Range, RangeBounds};
use std::{fmt, iter};

use crop::tree::{Arc, Node, Tree};
use smallvec::{SmallVec, smallvec};
use stdx::range::RangeExt;
use tinyset::SetU64;

pub use self::builder::MarkBuilder;
use self::key::{Flags, Key};
use self::leaf::Leaf;
use self::summary::ByteMetric;

/// The number of bits available for a mark id.
pub const ID_BITS: u32 = 61;

/// The largest mark id: ids must be at most this, the bits above it hold the mark's flags.
pub const MAX_ID: u64 = (1 << ID_BITS) - 1;

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

    fn get_left(&self, id: u64) -> Option<usize> {
        let (offset, leaf) = self.find_left_leaf(id)?;
        leaf.get_left(id).map(|byte| offset + byte)
    }

    fn get_right(&self, id: u64) -> Option<usize> {
        let (offset, leaf) = self.find_right_leaf(id)?;
        leaf.get_right(id).map(|byte| offset - byte)
    }

    /// Return the `(offset, leaf)` pair of the leftmost leaf that contains the given `id`.
    fn find_left_leaf(&self, id: u64) -> Option<(usize, &Leaf<N>)> {
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
    fn find_right_leaf(&self, id: u64) -> Option<(usize, &Leaf<N>)> {
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

    /// Replaces the bytes in `range` with `by` bytes. Marks inside the range move to its end,
    /// `start + by`, except left-biased marks at `start`, which stay there.
    pub fn shift(&mut self, range: impl RangeBounds<usize>, by: usize) {
        let (start, end) = range_bounds_to_start_end(range, 0, self.len());
        if !self.spans_leaves(start..end) {
            self.replace(start..end, Replacement::Gap(by));
            self.assert_invariants();
            return;
        }

        // `crop` drops the leaves between the first and last leaf of the range along with their
        // keys, so take the keys out of the range first and put them back after the edit.
        let mut left = SmallVec::<Key, 1>::new();
        let mut right = SmallVec::<Key, 1>::new();
        self.retain_keys(start..end, |at, key| {
            if !(start..=end).contains(&at) {
                return true;
            }
            if at == start && by > 0 && key.flags().contains(Flags::BIAS_LEFT) {
                left.push(key);
            } else {
                right.push(key);
            }
            false
        });

        self.replace(start..end, Replacement::Gap(by));
        if !left.is_empty() {
            self.replace(start..start, Replacement::Keys(left));
        }
        if !right.is_empty() {
            self.replace(start + by..start + by, Replacement::Keys(right));
        }
        self.assert_invariants();
    }

    /// Whether `crop` replaces `range` across more than one leaf. It picks the first child that
    /// ends at or after `range.start` at each level, as `Inode::child_containing_range` does.
    fn spans_leaves(&self, range: Range<usize>) -> bool {
        let Range { start, end } = range;
        let mut node = self.tree.root().as_ref();
        let mut offset = 0;
        loop {
            let inode = match node {
                Node::Internal(inode) => inode,
                Node::Leaf(_) => return false,
            };
            let child = inode
                .children()
                .iter()
                .find(|child| {
                    let bytes = child.summary().bytes;
                    if offset + bytes >= start {
                        true
                    } else {
                        offset += bytes;
                        false
                    }
                })
                .expect("range start is in bounds");
            if offset + child.summary().bytes < end {
                return true;
            }
            node = child.as_ref();
        }
    }

    /// Keeps only the keys in the leaves overlapping `range` for which `f(position, key)` returns
    /// true.
    fn retain_keys(&mut self, range: Range<usize>, mut f: impl FnMut(usize, Key) -> bool) {
        fn retain<const N: usize>(
            node: &mut Arc<Node<ARITY, Leaf<N>>>,
            mut offset: usize,
            range: &Range<usize>,
            f: &mut impl FnMut(usize, Key) -> bool,
        ) {
            match Arc::make_mut(node) {
                Node::Internal(inode) => {
                    for i in 0..inode.children().len() {
                        let bytes = inode.child(i).summary().bytes;
                        if (offset..offset + bytes).intersects(range) {
                            inode.with_child_mut(i, |child| retain(child, offset, range, f));
                        }
                        offset += bytes;
                    }
                }
                Node::Leaf(leaf) => {
                    leaf.value.retain(&mut leaf.summary, |at, key| f(offset + at, key));
                }
            }
        }

        retain(self.tree.root_mut(), 0, &range, &mut f);
    }

    /// Returns an iterator over the items whose start point is in the given range.
    pub fn range(
        &self,
        range: impl RangeBounds<usize>,
    ) -> impl Iterator<Item = (Range<usize>, Id)> + '_ {
        let (start, end) = self.position_range(range);
        let mut q = VecDeque::from([(0, self.tree.root().as_ref())]);

        let leaves = iter::from_fn(move || {
            while let Some((mut offset, node)) = q.pop_front() {
                match node {
                    Node::Internal(inode) => {
                        for child in inode.children() {
                            let summary = child.summary();
                            if (offset..offset + summary.bytes).intersects(&(start..end)) {
                                q.push_back((offset, child.as_ref()));
                            }
                            offset += summary.bytes;
                        }
                    }
                    Node::Leaf(leaf) => return Some((offset, leaf)),
                }
            }
            None
        });

        leaves.flat_map(move |(offset, leaf)| {
            let mut items = SmallVec::<(usize, Option<usize>, u64), 16>::new();
            let mut at = offset;
            for extent in leaf.as_slice().extents {
                if (start..end).contains(&at) {
                    for key in extent.keys().filter(|key| !key.is_end()) {
                        items.push((at, (!key.is_range()).then_some(at), key.id()));
                    }
                }
                for key in extent.keys().filter(|key| key.is_end()) {
                    if let Some((_, right, _)) =
                        items.iter_mut().find(|(_, _, item_id)| *item_id == key.id())
                    {
                        *right = Some(at);
                    }
                }
                at += extent.len();
            }

            items.into_iter().map(move |(left, right, id)| {
                let right = right.unwrap_or_else(|| {
                    self.get_right(id).expect("we should at least find the current key")
                });
                (left..right, id.into())
            })
        })
    }

    /// Inserts an item based on its byte position.
    /// This does not affect `self.len()`.
    pub fn insert(&mut self, at: usize, id: Id) -> Inserter<'_, Id, N> {
        Inserter { tree: self, id, builder: MarkBuilder::new(at) }
    }

    /// Removes the items whose start point is in the given range (the same items `range` returns)
    /// and returns them.
    pub fn drain(&mut self, range: impl RangeBounds<usize>) -> Drain<'_, Id, N> {
        let (start, end) = self.position_range(range);
        let items = self.range(start..end).collect::<Vec<_>>();
        if !items.is_empty() {
            let ids = items.iter().map(|&(_, id)| id.into()).collect::<SetU64>();
            // One pass over the leaves in the range removes both keys of point marks and of range
            // marks that end inside the range.
            self.retain_keys(start..end, |_, key| !ids.contains(key.id()));
            // A range mark that ends past the range still has its end key in a later leaf, so the
            // id is still in the tree. Only those marks fall back to a per-id delete.
            for id in ids {
                if self.tree.summary().ids.contains(id) {
                    self.delete(id).unwrap();
                }
            }
            self.assert_invariants();
        }

        Drain { items: items.into_iter(), _tree: PhantomData }
    }

    pub fn delete(&mut self, id: impl Into<Id>) -> Option<Range<usize>> {
        fn del<const N: usize>(
            node: &mut Arc<Node<ARITY, Leaf<N>>>,
            mut offset: usize,
            id: u64,
        ) -> (usize, Option<usize>) {
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
                    let start =
                        leaf.value.delete(&mut leaf.summary, id).expect("bitmap said it's here");
                    let end = leaf.summary.ids.contains(id).then(|| {
                        leaf.value.delete(&mut leaf.summary, id).expect("bitmap said it's here")
                    });
                    (offset + start, end.map(|end| offset + end))
                }
            }
        }

        let id = id.into().into();
        let root = self.tree.root_mut();
        if !root.summary().ids.contains(id) {
            return None;
        }

        let (start, end) = match del(root, 0, id) {
            // A range mark with both keys in one leaf: both deleted in that descent.
            (start, Some(end)) => (start, end),
            // A range mark whose end key is in a later leaf: descend again for it.
            (start, None) if root.summary().ids.contains(id) => {
                let (end, _) = del(root, 0, id);
                (start, end)
            }
            // A point mark has a single key (no `Flags::RANGE`), so it's gone after one delete.
            (start, None) => (start, start),
        };

        self.assert_invariants();
        Some(start..end)
    }

    /// Marks can be at `0..=len`, so an unbounded end includes a mark at `len`.
    fn position_range(&self, range: impl RangeBounds<usize>) -> (usize, usize) {
        range_bounds_to_start_end(range, 0, self.len() + 1)
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
        self.tree.assert_invariants();
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

        self.tree
            .replace(at..at, Replacement::Keys(smallvec![Key::new(id, self.builder.start_flags)]));
        assert_eq!(self.tree.len(), n, "first insertion should not change the length of the tree");

        if self.builder.start_flags.contains(Flags::RANGE) {
            assert!(self.builder.end_flags.contains(Flags::RANGE | Flags::END));
            let at = at + self.builder.width;
            self.tree.replace(
                at..at,
                Replacement::Keys(smallvec![Key::new(id, self.builder.end_flags)]),
            );
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
    // Invariant, `Keys` can only be used as a replacement if the range is empty.
    // The replacement range is `byte..byte`
    Keys(SmallVec<Key, 1>),
}

impl Replacement {
    fn width(&self) -> usize {
        match self {
            Replacement::Gap(n) => *n,
            Replacement::Keys(_) => 0,
        }
    }
}

pub struct Drain<'a, Id: MarkTreeId, const N: usize> {
    items: std::vec::IntoIter<(Range<usize>, Id)>,
    _tree: PhantomData<&'a mut MarkTree<Id, N>>,
}

impl<Id: MarkTreeId, const N: usize> Iterator for Drain<'_, Id, N> {
    type Item = (Range<usize>, Id);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.items.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.items.size_hint()
    }
}

impl<Id: MarkTreeId, const N: usize> ExactSizeIterator for Drain<'_, Id, N> {}

struct DebugSet<F>(F);

impl<F, I> fmt::Debug for DebugSet<F>
where
    F: Fn() -> I,
    I: IntoIterator<Item: fmt::Debug>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries((self.0)()).finish()
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
