use super::{Editor, Selector};
use crate::{BufferId, Mark, MarkBuilder, MarkId};

impl Editor {
    #[inline]
    pub fn marks(&self, selector: impl Selector<BufferId>) -> impl Iterator<Item = &Mark> {
        self.buffer(selector).marks()
    }

    #[inline]
    pub fn create_mark(
        &mut self,
        selector: impl Selector<BufferId>,
        builder: MarkBuilder,
    ) -> MarkId {
        self.buffer_mut(selector).create_mark(builder)
    }

    pub fn delete_mark(&mut self, selector: impl Selector<BufferId>, mark_id: MarkId) {
        self.buffer_mut(selector).delete_mark(mark_id);
    }
}
