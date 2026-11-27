use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use wasmtime::component::{Resource, ResourceTable};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use zi::{BufferId, Client, Editor, ViewId};

use crate::wit::zi::api;

pub(super) struct HostState {
    client: Client,
    wasi: WasiCtx,
    table: ResourceTable,
    // Need some mappings from wasm ui32 wasm resource rep to slotmap u64 keys.
    views: HashMap<u32, ViewId>,
    buffers: HashMap<u32, BufferId>,
    next_rep: AtomicU32,
}

impl HostState {
    pub(super) fn new(client: Client) -> Self {
        Self {
            client,
            wasi: WasiCtx::builder().build(),
            table: ResourceTable::new(),
            views: HashMap::new(),
            buffers: HashMap::new(),
            next_rep: AtomicU32::new(1),
        }
    }

    pub(super) fn client(&self) -> &Client {
        &self.client
    }

    pub(super) fn push_view(&mut self, id: ViewId) -> Resource<api::editor::View> {
        let rep = self.next_rep.fetch_add(1, Ordering::Relaxed);
        self.views.insert(rep, id);
        Resource::new_own(rep)
    }

    pub(super) fn view_id(&self, res: &Resource<api::editor::View>) -> wasmtime::Result<ViewId> {
        self.views
            .get(&res.rep())
            .copied()
            .ok_or_else(|| wasmtime::Error::msg("unknown view resource"))
    }

    pub(super) fn buffer_id(
        &self,
        res: &Resource<api::editor::Buffer>,
    ) -> wasmtime::Result<BufferId> {
        self.buffers
            .get(&res.rep())
            .copied()
            .ok_or_else(|| wasmtime::Error::msg("unknown buffer resource"))
    }

    pub(super) async fn with_view<T: Send + 'static>(
        &mut self,
        res: &Resource<api::editor::View>,
        f: impl FnOnce(&mut Editor, ViewId) -> T + Send + 'static,
    ) -> wasmtime::Result<T> {
        let view = self.view_id(res)?;
        self.client
            .with(move |editor| editor.has_view(view).then(|| f(editor, view)))
            .await
            .ok_or_else(|| wasmtime::Error::msg(format!("view {view:?} no longer exists")))
    }

    pub(super) async fn with_buffer<T: Send + 'static>(
        &mut self,
        res: &Resource<api::editor::Buffer>,
        f: impl FnOnce(&mut Editor, BufferId) -> T + Send + 'static,
    ) -> wasmtime::Result<T> {
        let buf = self.buffer_id(res)?;
        self.client
            .with(move |editor| editor.has_buffer(buf).then(|| f(editor, buf)))
            .await
            .ok_or_else(|| wasmtime::Error::msg(format!("buffer {buf:?} no longer exists")))
    }

    pub(super) fn forget_view(&mut self, res: &Resource<api::editor::View>) {
        self.views.remove(&res.rep());
    }

    pub(super) fn forget_buffer(&mut self, res: &Resource<api::editor::Buffer>) {
        self.buffers.remove(&res.rep());
    }

    pub(super) fn push_buffer(&mut self, id: BufferId) -> Resource<api::editor::Buffer> {
        let rep = self.next_rep.fetch_add(1, Ordering::Relaxed);
        self.buffers.insert(rep, id);
        Resource::new_own(rep)
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}
