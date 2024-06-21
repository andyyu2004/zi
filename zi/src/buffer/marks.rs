use slotmap::SlotMap;
use zi_core::Point;

use super::Buffer;

slotmap::new_key_type! {
    pub struct MarkId;
}

impl Buffer {
    pub(crate) fn create_mark(&mut self, builder: MarkBuilder) -> MarkId {
        self.marks.tree.insert_with_key(|id| builder.build(id))
    }
}

#[derive(Default)]
pub(crate) struct Marks {
    // TODO need some smarter data structure here
    tree: SlotMap<MarkId, Mark>,
}

pub struct MarkBuilder {
    point: Point,
}

impl MarkBuilder {
    fn build(self, id: MarkId) -> Mark {
        Mark { id, point: self.point }
    }
}

pub struct Mark {
    id: MarkId,
    point: Point,
}

impl Mark {
    pub fn builder(point: Point) -> MarkBuilder {
        MarkBuilder { point }
    }
}
