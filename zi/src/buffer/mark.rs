use std::cell::Cell;
use std::fmt;
use std::ops::RangeBounds;

use slotmap::{Key, SlotMap};
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

    pub(crate) fn marks(&self, range: impl RangeBounds<usize>) -> impl Iterator<Item = &Mark> + '_ {
        self.marks.iter(range)
    }
}

#[derive(Debug)]
pub(crate) struct Marks {
    marks: SlotMap<MarkId, Mark>,
    // TODO pick some less arbitrary number
    tree: MarkTree<MarkIdWrapper, 32>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct MarkIdWrapper(MarkId);

impl fmt::Debug for MarkIdWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<MarkIdWrapper> for u64 {
    fn from(wrapper: MarkIdWrapper) -> u64 {
        wrapper.0.data().as_ffi()
    }
}

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
        self.tree.insert(byte, MarkIdWrapper(id));
        id
    }

    pub fn delete(&mut self, mark_id: MarkId) -> Option<Mark> {
        let mark = self.marks.remove(mark_id)?;
        let (byte, MarkIdWrapper(id)) = self
            .tree
            .delete(MarkIdWrapper(mark_id))
            .expect("if map contains mark, tree should too");
        debug_assert_eq!(id, mark_id);
        mark.byte.set(byte);
        Some(mark)
    }

    #[inline]
    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        self.tree.edit(deltas);
    }

    pub fn iter(&self, range: impl RangeBounds<usize>) -> impl Iterator<Item = &Mark> + '_ {
        self.tree.items(range).map(move |(byte, MarkIdWrapper(id))| {
            let mark = &self.marks[*id];
            mark.byte.set(byte);
            mark
        })
    }
}

pub struct MarkBuilder {
    byte: usize,
}

impl MarkBuilder {
    #[inline]
    fn build(self, id: MarkId) -> Mark {
        Mark { id, byte: Cell::new(self.byte) }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Mark {
    byte: Cell<usize>,
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

    #[inline]
    pub fn byte(&self) -> usize {
        self.byte.get()
    }
}
