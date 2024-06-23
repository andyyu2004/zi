use std::cell::Cell;

use slotmap::{Key, SlotMap};
use zi_text::{Deltas, MarkTree, MarkTreeItem};

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

    pub(crate) fn marks(&self) -> impl Iterator<Item = &Mark> + '_ {
        self.marks.iter()
    }
}

#[derive(Debug)]
pub(crate) struct Marks {
    marks: SlotMap<MarkId, Mark>,
    // TODO pick some less arbitrary number
    tree: MarkTree<MarkItem, 32>,
}

/// An entry in the mark tree. Only contains position information and a id reference to the mark data.
#[derive(Debug, Copy, Clone)]
struct MarkItem {
    byte: usize,
    id: MarkId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct MarkIdWrapper(MarkId);

impl From<MarkIdWrapper> for u64 {
    fn from(wrapper: MarkIdWrapper) -> u64 {
        wrapper.0.data().as_ffi()
    }
}

impl MarkTreeItem for MarkItem {
    type Id = MarkIdWrapper;

    #[inline]
    fn id(&self) -> Self::Id {
        MarkIdWrapper(self.id)
    }

    #[inline]
    fn byte(&self) -> usize {
        self.byte
    }

    #[inline]
    fn at(&self, byte: usize) -> Self {
        MarkItem { byte, ..*self }
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
        let item = MarkItem { byte, id };
        self.tree.insert(item);
        id
    }

    pub fn delete(&mut self, mark_id: MarkId) -> Option<Mark> {
        let mark = self.marks.remove(mark_id)?;
        let item = self
            .tree
            .delete(MarkIdWrapper(mark_id))
            .expect("if map contains mark, tree should too");
        debug_assert_eq!(item.id, mark_id);
        mark.byte.set(item.byte);
        Some(mark)
    }

    #[inline]
    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        self.tree.edit(deltas);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Mark> + '_ {
        self.tree.iter().map(move |item| {
            let mark = &self.marks[item.id];
            mark.byte.set(item.byte);
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
