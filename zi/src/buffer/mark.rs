use slotmap::SlotMap;
use zi_text::{Deltas, MarkTree, MarkTreeItem};

use super::Buffer;

slotmap::new_key_type! {
    pub struct MarkId;
}

impl Buffer {
    pub(crate) fn create_mark(&mut self, builder: MarkBuilder) -> MarkId {
        self.marks.insert(builder)
    }
}

#[derive(Debug)]
pub(crate) struct Marks {
    marks: SlotMap<MarkId, Mark>,
    // TODO pick some less arbitrary number
    tree: MarkTree<MarkItem, 32>,
}

#[derive(Debug, Copy, Clone)]
struct MarkItem {
    byte: usize,
    id: MarkId,
}

impl MarkTreeItem for MarkItem {
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

    pub fn insert(&mut self, builder: MarkBuilder) -> MarkId {
        let byte = builder.byte;
        let id = self.marks.insert_with_key(|id| builder.build(id));
        let item = MarkItem { byte, id };
        self.tree.insert(item);
        id
    }

    #[inline]
    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        self.tree.edit(deltas);
    }
}

pub struct MarkBuilder {
    byte: usize,
}

impl MarkBuilder {
    fn build(self, id: MarkId) -> Mark {
        Mark { id, byte: self.byte }
    }
}

#[derive(Debug)]
pub struct Mark {
    byte: usize,
    id: MarkId,
}

impl Mark {
    pub fn builder(byte: usize) -> MarkBuilder {
        MarkBuilder { byte }
    }
}
