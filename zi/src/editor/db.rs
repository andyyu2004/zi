use slotmap::SlotMap;

use crate::buffer::Buffer;
use crate::private::Internal;
use crate::view::ViewGroup;
use crate::{BufferId, Namespace, NamespaceId, View, ViewGroupId, ViewId};

#[derive(Default)]
pub(crate) struct Db {
    pub(crate) buffers: SlotMap<BufferId, Buffer>,
    pub(crate) views: SlotMap<ViewId, View>,
    pub(crate) view_groups: SlotMap<ViewGroupId, ViewGroup>,
    pub(crate) namespaces: SlotMap<NamespaceId, Namespace>,
}

impl Db {
    #[inline]
    pub(crate) fn view_and_buffer(&self, view: ViewId) -> (&View, &Buffer) {
        let view = &self.views[view];
        (view, &self.buffers[view.buffer()])
    }

    #[inline]
    pub(crate) fn view_and_buffer_mut(&mut self, view: ViewId) -> (&mut View, &mut Buffer) {
        let view = &mut self.views[view];
        let buf = view.buffer();
        (view, &mut self.buffers[buf])
    }

    #[inline]
    pub(crate) fn view_and_buffer_as<B: 'static>(&mut self, view: ViewId) -> (&mut View, &mut B) {
        let view = &mut self.views[view];
        let buf = self.buffers[view.buffer()]
            .as_any_mut(Internal(()))
            .downcast_mut::<B>()
            .expect("buffer downcast failed");
        (view, buf)
    }
}
