use std::ops::{Range, RangeBounds};

use slotmap::{Key, KeyData, SlotMap};
use zi_marktree::{MarkTree, MarkTreeId};
use zi_text::Deltas;

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
    ) -> impl Iterator<Item = (Range<usize>, &Mark)> + '_ {
        self.marks.iter(range)
    }
}

#[derive(Debug)]
pub(crate) struct Marks {
    marks: SlotMap<MarkId, Mark>,
    // TODO pick some less arbitrary number
    tree: MarkTree<MarkId, 128>,
}

impl From<u64> for MarkId {
    #[inline]
    fn from(id: u64) -> MarkId {
        MarkId::from(KeyData::from_ffi(id))
    }
}

impl From<MarkId> for u64 {
    #[inline]
    fn from(id: MarkId) -> u64 {
        id.data().as_ffi()
    }
}

// This is ok in practice since the upper 32 bits are used to store versions which is
// never going to be that high.
impl MarkTreeId for MarkId {}

impl Marks {
    pub(crate) fn new(n: usize) -> Self {
        Marks { marks: SlotMap::default(), tree: MarkTree::new(n) }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tree.len()
    }

    pub fn create(&mut self, builder: MarkBuilder) -> MarkId {
        let byte = builder.byte;
        let id = self.marks.insert_with_key(|id| builder.build(id));
        self.tree.insert(byte, id);
        id
    }

    pub fn delete(&mut self, id: MarkId) -> Option<(Range<usize>, Mark)> {
        let mark = self.marks.remove(id)?;
        let byte = self.tree.delete(id).expect("if map contains mark, tree should too");
        Some((byte, mark))
    }

    #[inline]
    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        deltas.iter().for_each(|delta| self.tree.shift(delta.range(), delta.text().len()));
    }

    #[inline]
    pub fn iter(
        &self,
        range: impl RangeBounds<usize>,
    ) -> impl Iterator<Item = (Range<usize>, &Mark)> + '_ {
        self.tree.range(range).map(move |(range, id)| (range, &self.marks[id]))
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
