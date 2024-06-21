mod marktree;
use slotmap::SlotMap;

use self::marktree::MarkTree;
use super::Buffer;

slotmap::new_key_type! {
    pub struct MarkId;
}

impl Buffer {
    pub(crate) fn create_mark(&mut self, builder: MarkBuilder) -> MarkId {
        let byte = builder.byte;
        let id = self.marks.marks.insert_with_key(|id| builder.build(id));
        let item = MarkItem { byte, id };
        self.marks.tree.insert(item);
        id
    }
}

#[derive(Default)]
pub(crate) struct Marks {
    marks: SlotMap<MarkId, Mark>,
    // TODO some less random number
    tree: MarkTree<32, MarkItem>,
}

#[derive(Debug, Copy, Clone)]
struct MarkItem {
    byte: usize,
    id: MarkId,
}

impl marktree::Item for MarkItem {
    #[inline]
    fn byte(&self) -> usize {
        self.byte
    }
}

impl Marks {}

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

#[cfg(test)]
mod tests;
