use std::collections::HashMap;
use std::ops::{Range, RangeBounds};

use itertools::Itertools;
use slotmap::{Key, KeyData, SlotMap};
use zi_marktree::{Bias, MarkTree, MarkTreeId};
use zi_text::Deltas;

use super::Buffer;
use crate::syntax::HighlightId;
use crate::NamespaceId;

slotmap::new_key_type! {
    pub struct MarkId;
}

impl Buffer {
    pub(crate) fn create_mark(&mut self, namespace: NamespaceId, builder: MarkBuilder) -> MarkId {
        let n = self.text().len_bytes();
        self.marks.create(n, namespace, builder)
    }

    pub(crate) fn create_marks(
        &mut self,
        namespace: NamespaceId,
        builders: impl IntoIterator<Item = MarkBuilder>,
    ) {
        self.marks.create_many(self.text().len_bytes(), namespace, builders)
    }

    pub(crate) fn clear_marks(&mut self, ns: NamespaceId, range: impl RangeBounds<usize>) {
        self.marks.drain(ns, range)
    }

    pub(crate) fn delete_mark(&mut self, ns: NamespaceId, id: MarkId) {
        self.marks.delete(ns, id);
    }

    pub(crate) fn marks(
        &self,
        range: impl RangeBounds<usize>,
    ) -> impl Iterator<Item = (Range<usize>, &Mark)> + '_ {
        self.marks.iter(range)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Marks {
    namespaces: HashMap<NamespaceId, PerNs>,
}

#[derive(Debug)]
struct PerNs {
    marks: SlotMap<MarkId, Mark>,
    // TODO pick some less arbitrary number
    tree: MarkTree<MarkId, 32>,
}

impl PerNs {
    fn new(text_len: usize) -> Self {
        Self { marks: SlotMap::default(), tree: MarkTree::new(text_len + 1) }
    }
    fn iter(
        &self,
        range: impl RangeBounds<usize>,
    ) -> impl Iterator<Item = (Range<usize>, &Mark)> + '_ {
        self.tree.range(range).map(move |(range, id)| (range, &self.marks[id]))
    }

    fn edit(&mut self, deltas: &Deltas<'_>) {
        deltas.iter().for_each(|delta| self.tree.shift(delta.range(), delta.text().len()));
    }

    fn create(&mut self, builder: MarkBuilder) -> MarkId {
        let MarkBuilder { start_bias, end_bias, byte, width, hl: _ } = builder;
        let id = self.marks.insert_with_key(|id| builder.build(id));
        self.tree.insert(byte, id).width(width).start_bias(start_bias).end_bias(end_bias);
        id
    }

    fn delete(&mut self, id: MarkId) -> Option<(Range<usize>, Mark)> {
        let mark = self.marks.remove(id)?;
        let range = self.tree.delete(id).expect("if map contains mark, tree should too");
        Some((range, mark))
    }

    fn drain<'a>(&'a mut self, range: impl RangeBounds<usize>) {
        for (_range, id) in self.tree.drain(range) {
            self.marks.remove(id).unwrap();
        }
    }
}

/// We're packing the 64bit `slotmap::KeyData` into 32 bits.
/// Usually the upper 32-bits are for the version and the lower 32-bits are for the index.
/// In practice, 12-bits should be enough for the version, and 20-bits for the index.
impl MarkTreeId for MarkId {}

impl From<u32> for MarkId {
    #[inline]
    fn from(raw: u32) -> MarkId {
        let version = raw >> 20;
        let index = raw << 12 >> 12;
        MarkId::from(KeyData::from_ffi((version as u64) << 32 | index as u64))
    }
}

impl From<MarkId> for u32 {
    #[inline]
    fn from(id: MarkId) -> u32 {
        let raw = id.data().as_ffi();
        let version = raw >> 32;
        let index = raw << 32 >> 32;

        assert!(version < 1 << 12, "version is too large");
        assert!(index < 1 << 20, "index is too large");

        (version << 20 | index) as u32
    }
}

impl Marks {
    pub(crate) fn create(
        &mut self,
        text_len: usize,
        namespace: NamespaceId,
        builder: MarkBuilder,
    ) -> MarkId {
        debug_assert!(self.namespaces.values().all(|per_ns| per_ns.tree.len() == text_len + 1));
        self.namespaces.entry(namespace).or_insert_with(|| PerNs::new(text_len)).create(builder)
    }

    pub(crate) fn create_many(
        &mut self,
        text_len: usize,
        namespace: NamespaceId,
        builders: impl IntoIterator<Item = MarkBuilder>,
    ) {
        debug_assert!(self.namespaces.values().all(|per_ns| per_ns.tree.len() == text_len + 1));
        let per_ns = self.namespaces.entry(namespace).or_insert_with(|| PerNs::new(text_len));
        builders.into_iter().for_each(|builder| {
            per_ns.create(builder);
        });
    }

    pub fn delete(&mut self, ns: NamespaceId, id: MarkId) -> Option<(Range<usize>, Mark)> {
        self.namespaces.get_mut(&ns).and_then(|ns| ns.delete(id))
    }

    pub fn drain(&mut self, ns: NamespaceId, range: impl RangeBounds<usize>) {
        if let Some(per_ns) = self.namespaces.get_mut(&ns) {
            per_ns.drain(range)
        }
    }

    #[inline]
    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        for ns in self.namespaces.values_mut() {
            ns.edit(deltas)
        }
    }

    /// Returns an iterator over all marks in the given range (all namespaces) sorted by `start`.
    #[inline]
    pub fn iter(
        &self,
        range: impl RangeBounds<usize>,
    ) -> impl Iterator<Item = (Range<usize>, &Mark)> + '_ {
        let range = (range.start_bound().cloned(), range.end_bound().cloned());
        self.namespaces
            .values()
            .map(|per_ns| per_ns.iter(range))
            .kmerge_by(|(a, _), (b, _)| a.start < b.start)
    }
}

pub struct MarkBuilder {
    hl: HighlightId,
    byte: usize,
    width: usize,
    start_bias: Bias,
    end_bias: Bias,
}

impl MarkBuilder {
    pub fn hl(mut self, hl: HighlightId) -> Self {
        self.hl = hl;
        self
    }

    pub fn width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }

    pub fn start_bias(mut self, bias: Bias) -> Self {
        self.start_bias = bias;
        self
    }

    pub fn end_bias(mut self, bias: Bias) -> Self {
        self.end_bias = bias;
        self
    }

    #[inline]
    fn build(self, id: MarkId) -> Mark {
        Mark { id, hl: self.hl }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Mark {
    id: MarkId,
    hl: HighlightId,
}

impl Mark {
    #[inline]
    pub fn builder(byte: usize) -> MarkBuilder {
        MarkBuilder {
            byte,
            width: 0,
            start_bias: Bias::Right,
            end_bias: Bias::Right,
            hl: Default::default(),
        }
    }

    #[inline]
    pub fn id(&self) -> MarkId {
        self.id
    }

    #[inline]
    pub fn highlight(&self) -> HighlightId {
        self.hl
    }
}
