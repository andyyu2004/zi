use std::ops::{Range, RangeBounds};

use super::{Editor, Selector};
use crate::{BufferId, Mark, MarkBuilder, MarkId, NamespaceId};

impl Editor {
    #[inline]
    pub fn marks(
        &self,
        selector: impl Selector<BufferId>,
        byte_range: impl RangeBounds<usize>,
    ) -> impl Iterator<Item = (NamespaceId, Range<usize>, &Mark)> {
        self.buffer(selector).marks(byte_range)
    }

    pub fn clear_marks(
        &mut self,
        selector: impl Selector<BufferId>,
        namespace: impl Selector<NamespaceId>,
        range: impl RangeBounds<usize>,
    ) {
        let ns = namespace.select(self);
        self.buffer_mut(selector).clear_marks(ns, range)
    }

    #[inline]
    pub fn create_mark(
        &mut self,
        selector: impl Selector<BufferId>,
        namespace: impl Selector<NamespaceId>,
        builder: MarkBuilder,
    ) -> MarkId {
        let namespace = namespace.select(self);
        self.buffer_mut(selector).create_mark(namespace, builder)
    }

    #[inline]
    pub fn delete_mark(
        &mut self,
        selector: impl Selector<BufferId>,
        namespace: impl Selector<NamespaceId>,
        mark: MarkId,
    ) {
        let namespace = namespace.select(self);
        self.buffer_mut(selector).delete_mark(namespace, mark);
    }
}
