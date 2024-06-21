mod marktree;
use slotmap::SlotMap;
use zi_core::Point;

use self::marktree::MarkTree;
use super::Buffer;

slotmap::new_key_type! {
    pub struct MarkId;
}

impl Buffer {
    pub(crate) fn create_mark(&mut self, builder: MarkBuilder) -> MarkId {
        let id = self.marks.marks.insert_with_key(|id| builder.build(id));
        id
    }
}

#[derive(Default)]
pub(crate) struct Marks {
    marks: SlotMap<MarkId, Mark>,
    tree: MarkTree,
}

impl Marks {}

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
