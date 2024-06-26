use std::fmt;
use std::ops::RangeBounds;

use slotmap::{Key, KeyData, SlotMap};
use zi_text::{Deltas, MarkTree};

use super::Buffer;

slotmap::new_key_type! {
    pub struct MarkId;
}

impl Buffer {
    pub(crate) fn create_mark(&mut self, builder: MarkBuilder) -> MarkId {
        self.marks.create(builder)
    }

    pub(crate) fn delete_mark(&mut self, mark_id: MarkId) {
        self.marks.delete(mark_id);
    }

    pub(crate) fn marks(
        &self,
        range: impl RangeBounds<usize>,
    ) -> impl Iterator<Item = (usize, &Mark)> + '_ {
        self.marks.iter(range)
    }
}

#[derive(Debug)]
pub(crate) struct Marks {
    marks: SlotMap<MarkId, Mark>,
    // TODO pick some less arbitrary number
    tree: MarkTree<IdWrapper, 128>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct IdWrapper(MarkId);

impl fmt::Debug for IdWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<IdWrapper> for u64 {
    #[inline]
    fn from(wrapper: IdWrapper) -> u64 {
        wrapper.0.data().as_ffi()
    }
}

impl From<u64> for IdWrapper {
    #[inline]
    fn from(id: u64) -> IdWrapper {
        IdWrapper(MarkId::from(KeyData::from_ffi(id)))
    }
}

impl Marks {
    pub(crate) fn new(n: usize) -> Self {
        if n > 1024 * 1024 {}
        Marks { marks: SlotMap::default(), tree: MarkTree::new(n + 1) }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    pub fn create(&mut self, builder: MarkBuilder) -> MarkId {
        let byte = builder.byte;
        let id = self.marks.insert_with_key(|id| builder.build(id));
        self.tree.insert(byte, IdWrapper(id));
        id
    }

    pub fn delete(&mut self, id: MarkId) -> Option<(usize, Mark)> {
        let mark = self.marks.remove(id)?;
        let byte = self.tree.delete(IdWrapper(id)).expect("if map contains mark, tree should too");
        Some((byte, mark))
    }

    #[inline]
    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        self.tree.edit(deltas);
    }

    #[inline]
    pub fn iter(
        &self,
        range: impl RangeBounds<usize>,
    ) -> impl Iterator<Item = (usize, &Mark)> + '_ {
        self.tree.range(range).map(move |(byte, IdWrapper(id))| (byte, &self.marks[id]))
    }
}

pub struct MarkBuilder {
    byte: usize,
}

impl MarkBuilder {
    #[inline]
    fn build(self, id: MarkId) -> Mark {
        Mark { id }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Mark {
    id: MarkId,
}

impl Mark {
    #[inline]
    pub fn builder(byte: usize) -> MarkBuilder {
        MarkBuilder { byte }
    }

    #[inline]
    pub fn id(&self) -> MarkId {
        self.id
    }
}
