use super::{Editor, Selector};
use crate::{BufferId, MarkBuilder, MarkId};

impl Editor {
    pub fn create_mark(
        &mut self,
        selector: impl Selector<BufferId>,
        builder: MarkBuilder,
    ) -> MarkId {
        let buf = selector.select(self);
        self[buf].create_mark(builder)
    }
}
