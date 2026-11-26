//! The editor's relations. Rows hold an editor object next to its relations to other rows.

use std::ops::Deref;
use std::path::PathBuf;

use rel::Relation;

use crate::editor::Selector;
use crate::private::Internal;
use crate::view::ViewGroup;
use crate::{BufferId, Editor, Namespace, ViewGroupId, ViewId};

#[derive(Default, rel::Database)]
pub(crate) struct Db {
    pub(crate) buffers: Relation<Buffer>,
    pub(crate) views: Relation<View>,
    pub(crate) view_groups: Relation<ViewGroup>,
    pub(crate) namespaces: Relation<Namespace>,
}

/// A buffer's row. Its file path is fixed when it is created.
#[derive(Relation)]
#[relation(key = BufferId)]
pub struct Buffer {
    #[index]
    path: Option<PathBuf>,
    pub(crate) buffer: crate::buffer::Buffer,
}

impl Buffer {
    pub(crate) fn new(buffer: crate::buffer::Buffer) -> Self {
        Self { path: buffer.file_path(), buffer }
    }
}

impl Deref for Buffer {
    type Target = crate::buffer::Buffer;

    fn deref(&self) -> &crate::buffer::Buffer {
        &self.buffer
    }
}

/// A view's row: the buffer it shows and the group it is in, next to the view itself.
#[derive(Debug, Relation)]
#[relation(key = ViewId)]
pub struct View {
    #[references(Buffer, on_delete = restrict)]
    buf: BufferId,
    #[references(ViewGroup, on_delete = set_null)]
    group: Option<ViewGroupId>,
    pub(crate) view: crate::View,
}

impl View {
    /// The buffer that this view is displaying.
    #[inline]
    pub fn buffer(&self) -> BufferId {
        self.buf
    }

    #[inline]
    pub fn group(&self) -> Option<ViewGroupId> {
        self.group
    }
}

impl Deref for View {
    type Target = crate::View;

    fn deref(&self) -> &crate::View {
        &self.view
    }
}

impl Db {
    #[inline]
    pub(crate) fn view_and_buffer(&self, view: ViewId) -> (&View, &crate::buffer::Buffer) {
        let view = &self.views[view];
        (view, &self.buffers[view.buffer()].buffer)
    }

    #[inline]
    pub(crate) fn view_and_buffer_mut(
        &mut self,
        view: ViewId,
    ) -> (&mut crate::View, &mut crate::buffer::Buffer) {
        let view = self.views.get_mut(view).expect("bad view id");
        let buf = self.buffers.get_mut(*view.buf).expect("bad buffer id").buffer;
        (view.view, buf)
    }

    #[inline]
    pub(crate) fn view_and_buffer_as<B: 'static>(
        &mut self,
        view: ViewId,
    ) -> (&mut crate::View, &mut B) {
        let (view, buf) = self.view_and_buffer_mut(view);
        let buf = buf.as_any_mut(Internal(())).downcast_mut::<B>().expect("buffer downcast failed");
        (view, buf)
    }

    #[inline]
    pub(crate) fn buffer_mut(&mut self, buf: BufferId) -> &mut crate::buffer::Buffer {
        self.buffers.get_mut(buf).expect("bad buffer id").buffer
    }

    #[inline]
    pub(crate) fn view_mut(&mut self, view: ViewId) -> &mut crate::View {
        self.views.get_mut(view).expect("bad view id").view
    }

    pub(crate) fn insert_buffer(
        &mut self,
        f: impl FnOnce(BufferId) -> crate::buffer::Buffer,
    ) -> BufferId {
        self.buffers.insert_with(|id| Buffer::new(f(id)))
    }

    pub(crate) fn insert_view(
        &mut self,
        buf: BufferId,
        group: Option<ViewGroupId>,
        f: impl FnOnce(ViewId) -> crate::View,
    ) -> ViewId {
        rel::Database::insert_with(self, |id| View { buf, group, view: f(id) })
            .expect("bad buffer or view group id")
    }

    /// Show a different buffer in a view, starting from the top.
    pub(crate) fn set_buffer(&mut self, view: ViewId, buf: BufferId) {
        if self.views[view].buffer() != buf {
            rel::Database::set(self, view, self::view::buf, buf).expect("bad buffer id");
            self.view_mut(view).reset_position();
        }
    }

    pub(crate) fn set_group(&mut self, view: ViewId, group: ViewGroupId) {
        rel::Database::set(self, view, self::view::group, Some(group)).expect("bad view group id");
    }
}

impl Selector<ViewId> for View {
    #[inline]
    fn select(&self, _editor: &Editor) -> ViewId {
        self.view.id()
    }
}

impl Selector<BufferId> for View {
    #[inline]
    fn select(&self, _editor: &Editor) -> BufferId {
        self.buf
    }
}
