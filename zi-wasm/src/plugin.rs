use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use futures_util::future::BoxFuture;
use futures_util::{FutureExt, Stream, StreamExt, TryStreamExt};
use parking_lot::RwLock;
use slotmap::SlotMap;
use smol_str::SmolStr;
use tokio::select;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinSet;
use tokio_stream::wrappers::ReadDirStream;
pub use wasmtime::Engine;
use wasmtime::component::ResourceTable;
use wasmtime::component::{Component, Linker, Resource, ResourceAny};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use zi::command::{self, CommandRange, Handler, Word};
use zi::{
    Active, BufferId, Client, Editor, FileType, LanguageConfig, LanguageServiceId, Point,
    SaveFlags, ViewId, dirs,
};
use zi_lsp::LanguageServerConfig;

use crate::wit::Plugin;
use crate::wit::zi::api;

const EPOCH_TICK: Duration = Duration::from_millis(1);

pub fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true);
        config.epoch_interruption(true);
        Engine::new(&config).expect("configuration should be valid")
    })
}

fn new_store(engine: &Engine, state: HostState) -> Store {
    let mut store = Store::new(engine, state);
    store.epoch_deadline_async_yield_and_update(1);
    store
}

struct EpochTicker {
    stop: Arc<AtomicBool>,
}

impl EpochTicker {
    fn start(engine: &'static Engine) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let ticking = stop.clone();
        std::thread::Builder::new()
            .name("zi-wasm-epoch".into())
            .spawn(move || {
                while !ticking.load(Ordering::Relaxed) {
                    std::thread::sleep(EPOCH_TICK);
                    engine.increment_epoch();
                }
            })
            .expect("couldn't spawn the epoch ticker thread");
        Self { stop }
    }
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub struct HostState {
    client: Client,
    wasi: WasiCtx,
    table: ResourceTable,
    // Need some mappings from wasm ui32 wasm resource rep to slotmap u64 keys.
    views: HashMap<u32, ViewId>,
    buffers: HashMap<u32, BufferId>,
    next_rep: AtomicU32,
}

impl HostState {
    fn new(client: Client) -> Self {
        Self {
            client,
            wasi: WasiCtx::builder().build(),
            table: ResourceTable::new(),
            views: HashMap::new(),
            buffers: HashMap::new(),
            next_rep: AtomicU32::new(1),
        }
    }

    fn client(&self) -> &Client {
        &self.client
    }

    fn push_view(&mut self, id: ViewId) -> Resource<api::editor::View> {
        let rep = self.next_rep.fetch_add(1, Ordering::Relaxed);
        self.views.insert(rep, id);
        Resource::new_own(rep)
    }

    fn view_id(&self, res: &Resource<api::editor::View>) -> wasmtime::Result<ViewId> {
        self.views
            .get(&res.rep())
            .copied()
            .ok_or_else(|| wasmtime::Error::msg("unknown view resource"))
    }

    fn buffer_id(&self, res: &Resource<api::editor::Buffer>) -> wasmtime::Result<BufferId> {
        self.buffers
            .get(&res.rep())
            .copied()
            .ok_or_else(|| wasmtime::Error::msg("unknown buffer resource"))
    }

    async fn with_view<T: Send + 'static>(
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

    async fn with_buffer<T: Send + 'static>(
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

    fn push_buffer(&mut self, id: BufferId) -> Resource<api::editor::Buffer> {
        let rep = self.next_rep.fetch_add(1, Ordering::Relaxed);
        self.buffers.insert(rep, id);
        Resource::new_own(rep)
    }
}

pub type Store = wasmtime::Store<HostState>;

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

fn linker(engine: &Engine) -> wasmtime::Result<Linker<HostState>> {
    let mut linker = Linker::new(engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
    Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)?;
    Ok(linker)
}

impl api::editor::HostView for HostState {
    async fn exists(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<bool> {
        let view = self.view_id(&view)?;
        Ok(self.client.with(move |editor| editor.has_view(view)).await)
    }

    async fn get_buffer(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<Resource<api::editor::Buffer>> {
        let buf_id = self.with_view(&view, |editor, view| editor.view(view).buffer()).await?;
        Ok(self.push_buffer(buf_id))
    }

    async fn get_cursor(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<api::editor::Point> {
        self.with_view(&view, |editor, view| editor.view(view).cursor().into()).await
    }

    async fn set_cursor(
        &mut self,
        view: Resource<api::editor::View>,
        pos: api::editor::Point,
    ) -> wasmtime::Result<()> {
        self.with_view(&view, move |editor, view| {
            editor.set_cursor(view, Point::from(pos));
        })
        .await
    }

    async fn close(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<()> {
        let view_id = self.view_id(&view)?;
        command::close_view(&self.client, view_id).await;
        Ok(())
    }

    async fn save(
        &mut self,
        view: Resource<api::editor::View>,
        force: bool,
    ) -> wasmtime::Result<Result<(), String>> {
        let flags = if force { SaveFlags::FORCE } else { SaveFlags::empty() };
        let fut = self.with_view(&view, move |editor, view| editor.save(view, flags)).await?;
        Ok(fut.await.map_err(|e| e.to_string()))
    }

    async fn inspect(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<()> {
        self.with_view(&view, Editor::inspect).await
    }

    async fn insert(
        &mut self,
        view: Resource<api::editor::View>,
        text: String,
    ) -> wasmtime::Result<Result<(), api::editor::EditError>> {
        let res = self.with_view(&view, move |editor, view| editor.insert(view, &text)).await?;
        Ok(res.map_err(Into::into))
    }

    async fn delete_char(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<Result<(), api::editor::EditError>> {
        let res = self.with_view(&view, Editor::delete_char).await?;
        Ok(res.map_err(Into::into))
    }

    async fn paste_after(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<Result<(), api::editor::EditError>> {
        let res = self.with_view(&view, Editor::paste_after).await?;
        Ok(res.map_err(Into::into))
    }

    async fn undo(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<Result<bool, api::editor::EditError>> {
        let res = self.with_view(&view, Editor::undo).await?;
        Ok(res.map_err(Into::into))
    }

    async fn redo(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> wasmtime::Result<Result<bool, api::editor::EditError>> {
        let res = self.with_view(&view, Editor::redo).await?;
        Ok(res.map_err(Into::into))
    }

    async fn scroll(
        &mut self,
        view: Resource<api::editor::View>,
        direction: api::editor::Direction,
        amount: u32,
    ) -> wasmtime::Result<()> {
        let dir = zi::Direction::from(direction);
        self.with_view(&view, move |editor, view| editor.scroll(view, dir, amount as usize)).await
    }

    async fn align(
        &mut self,
        view: Resource<api::editor::View>,
        alignment: api::editor::VerticalAlignment,
    ) -> wasmtime::Result<()> {
        let align = zi::VerticalAlignment::from(alignment);
        self.with_view(&view, move |editor, view| editor.align_view(view, align)).await
    }

    async fn focus(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<()> {
        self.with_view(&view, Editor::focus).await
    }

    #[expect(clippy::unused_async_trait_impl, reason = "keep resource removal deferred until poll")]
    async fn drop(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<()> {
        self.views.remove(&view.rep());
        Ok(())
    }
}

impl api::editor::HostBuffer for HostState {
    async fn exists(&mut self, buf: Resource<api::editor::Buffer>) -> wasmtime::Result<bool> {
        let buf = self.buffer_id(&buf)?;
        Ok(self.client.with(move |editor| editor.has_buffer(buf)).await)
    }

    async fn get_file_path(
        &mut self,
        buf: Resource<api::editor::Buffer>,
    ) -> wasmtime::Result<Option<String>> {
        self.with_buffer(&buf, |editor, buf| {
            editor.buffer(buf).file_path().map(|p| p.to_string_lossy().into_owned())
        })
        .await
    }

    async fn get_flags(
        &mut self,
        buf: Resource<api::editor::Buffer>,
    ) -> wasmtime::Result<api::editor::BufferFlags> {
        self.with_buffer(&buf, |editor, buf| {
            let buf = editor.buffer(buf);
            let mut flags = api::editor::BufferFlags::empty();
            if buf.flags().contains(zi::BufferFlags::DIRTY) {
                flags |= api::editor::BufferFlags::DIRTY;
            }
            flags
        })
        .await
    }

    async fn len_lines(&mut self, buf: Resource<api::editor::Buffer>) -> wasmtime::Result<u32> {
        self.with_buffer(&buf, |editor, buf| editor.buffer(buf).text().len_lines() as u32).await
    }

    #[expect(clippy::unused_async_trait_impl, reason = "keep resource removal deferred until poll")]
    async fn drop(&mut self, buf: Resource<api::editor::Buffer>) -> wasmtime::Result<()> {
        self.buffers.remove(&buf.rep());
        Ok(())
    }
}

impl api::editor::Host for HostState {
    async fn insert(
        &mut self,
        text: String,
    ) -> wasmtime::Result<Result<(), api::editor::EditError>> {
        Ok(self.client.with(move |editor| editor.insert(Active, &text)).await.map_err(Into::into))
    }

    async fn get_mode(&mut self) -> wasmtime::Result<api::editor::Mode> {
        Ok(self.client.with(|editor| editor.mode()).await.into())
    }

    async fn set_mode(&mut self, mode: api::editor::Mode) -> wasmtime::Result<()> {
        self.client.with(move |editor| editor.set_mode(mode.into())).await;
        Ok(())
    }

    async fn get_active_view(&mut self) -> wasmtime::Result<Resource<api::editor::View>> {
        let view_id = self.client.with(|editor| editor.view(Active).id()).await;
        Ok(self.push_view(view_id))
    }

    async fn save_all(&mut self, force: bool) -> wasmtime::Result<Result<(), String>> {
        Ok(command::save_all(&self.client, force).await.map_err(|e| e.to_string()))
    }

    async fn reload(&mut self) -> wasmtime::Result<Result<(), String>> {
        Ok(command::reload(&self.client).await.map_err(|e| e.to_string()))
    }

    async fn open_jump_list(&mut self) -> wasmtime::Result<()> {
        self.client
            .with(|editor| {
                editor.open_jump_list(Active);
            })
            .await;
        Ok(())
    }

    async fn open_file_explorer(&mut self, path: String) -> wasmtime::Result<()> {
        self.client.with(move |editor| editor.open_file_explorer(path)).await;
        Ok(())
    }

    async fn set_option(
        &mut self,
        key: String,
        value: String,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self
            .client
            .with(move |editor| command::set_option(editor, &key, &value))
            .await
            .map_err(|e| e.to_string()))
    }

    async fn add_language_server(
        &mut self,
        id: String,
        server: api::editor::LanguageServer,
    ) -> wasmtime::Result<()> {
        let api::editor::LanguageServer { command, args } = server;
        let server = LanguageServerConfig::new(command, args.into_iter().map(Into::into));
        self.client
            .with(move |editor| {
                editor.language_config_mut().add_language_service(id.as_str(), server);
            })
            .await;
        Ok(())
    }

    async fn add_language(
        &mut self,
        filetype: String,
        language_servers: Vec<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self
            .client
            .with(move |editor| {
                let config = editor.language_config_mut();
                let ids = language_servers.iter().map(|id| LanguageServiceId::new(id.as_str()));
                if let Some(id) = ids.clone().find(|&id| !config.has_language_service(id)) {
                    return Err(format!(
                        "language server `{id}` for language `{filetype}` is not defined"
                    ));
                }
                let mut seen = std::collections::BTreeSet::new();
                if let Some(id) = ids.clone().find(|&id| !seen.insert(id)) {
                    return Err(format!(
                        "language server `{id}` is listed more than once for language `{filetype}`"
                    ));
                }
                config.add_language(FileType::from_name(&filetype), LanguageConfig::new(ids));
                Ok(())
            })
            .await)
    }
}

/// The plugin manager responsible for loading and running wasm plugins and keeping track of their state.
/// This also provides the interface for the editor to interact with the plugins.
pub struct PluginManager {
    plugins: RwLock<SlotMap<PluginId, PluginState>>,
    initialized: watch::Sender<bool>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self { plugins: Default::default(), initialized: watch::Sender::new(false) }
    }
}

impl PluginManager {
    pub fn initialized(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut initialized = self.initialized.subscribe();
        async move {
            let _ = initialized.wait_for(|&initialized| initialized).await;
        }
    }

    #[must_use]
    fn add(&self, _name: impl Into<SmolStr>, tx: Sender<PluginRequest>) -> PluginId {
        // TODO name uniqueness check?
        self.plugins.write().insert(PluginState { client: PluginClient(tx) })
    }

    fn with_plugin<F, T>(&self, id: PluginId, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&PluginState) -> T,
    {
        self.plugins
            .read()
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("plugin not found: {id:?}"))
            .map(f)
    }

    fn plugin_client(&self, id: PluginId) -> anyhow::Result<PluginClient> {
        self.with_plugin(id, |state| state.client.clone())
    }

    pub async fn execute(
        &self,
        id: PluginId,
        name: Word,
        range: Option<CommandRange>,
        args: Box<[Word]>,
        force: bool,
    ) -> anyhow::Result<()> {
        let client = self.plugin_client(id)?;
        client.execute(name, range, args, force).await?;
        Ok(())
    }
}

slotmap::new_key_type! {
    pub struct PluginId;
}

#[derive(Clone)]
struct PluginClient(Sender<PluginRequest>);

impl PluginClient {
    pub async fn execute(
        &self,
        name: Word,
        range: Option<CommandRange>,
        args: Box<[Word]>,
        force: bool,
    ) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.0.send(PluginRequest::ExecuteCommand { name, range, args, force, tx }).await?;
        rx.await?
    }
}

struct PluginState {
    client: PluginClient,
}

type Responder<T> = oneshot::Sender<anyhow::Result<T>>;

enum PluginRequest {
    ExecuteCommand {
        name: Word,
        range: Option<CommandRange>,
        args: Box<[Word]>,
        force: bool,
        tx: Responder<()>,
    },
}

#[async_trait::async_trait]
impl zi::plugin::PluginManager for PluginManager {
    fn name(&self) -> &'static str {
        "wasm"
    }

    async fn start(self: Arc<Self>, client: Client) -> anyhow::Result<()> {
        let engine = engine();

        let components = Self::load_plugin_components(engine);

        let mut join_set = JoinSet::new();

        let plugin_hosts = components
            .then(|component| {
                let plugins = self.clone();
                let client = client.clone();
                async move {
                    let component = component?;
                    let linker = linker(engine)?;
                    let mut store = new_store(engine, HostState::new(client));
                    let plugin = Plugin::instantiate_async(&mut store, &component, &linker).await?;

                    Ok::<_, anyhow::Error>(PluginHost::new(plugins, store, plugin))
                }
            })
            .filter_map(|res| async {
                match res {
                    Ok(plugin) => Some(plugin),
                    Err(err) => {
                        tracing::error!(error = &*err, "error loading plugin (skipping)");
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_boxed_slice();

        tracing::info!(n = plugin_hosts.len(), "instantiated plugins");

        let timeout = client.with(|editor| *editor.settings().plugin_init_timeout.read()).await;
        for (host, receiver) in initialize_all(plugin_hosts.into_vec(), timeout).await {
            join_set.spawn(host.run(receiver));
        }

        self.initialized.send_replace(true);

        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(Ok(())) => {}
                Ok(Err(err)) => tracing::error!(error = &*err, "error running plugin"),
                Err(err) => {
                    tracing::error!(error = &err as &dyn std::error::Error, "error joining plugin");
                }
            }
        }

        Ok(())
    }
}

impl PluginManager {
    fn load_plugin_components(
        engine: &'static Engine,
    ) -> impl Stream<Item = anyhow::Result<Component>> {
        let dirs = futures_util::stream::iter(dirs::plugin());
        let entries = dirs
            .then(|dir| async move {
                let entries = tokio::fs::read_dir(dir).await?;
                Ok::<_, io::Error>(ReadDirStream::new(entries))
            })
            .try_flatten();

        entries.map_err(Into::into).try_filter_map(move |entry| async move {
            if !entry.file_type().await?.is_file() {
                return Ok(None);
            }

            let component = Component::from_file(engine, entry.path())?;
            Ok(Some(component))
        })
    }
}

async fn initialize_all(
    hosts: Vec<PluginHost>,
    timeout: Duration,
) -> Vec<(PluginHost, mpsc::Receiver<PluginRequest>)> {
    let _ticker = EpochTicker::start(engine());
    let results = futures_util::future::join_all(hosts.into_iter().map(|mut host| async move {
        let start = Instant::now();
        let result = tokio::time::timeout(timeout, host.initialize()).await;
        let name = host.name.clone().unwrap_or_else(|| "<unnamed>".into());
        match result {
            Ok(Ok(receiver)) => {
                tracing::debug!(plugin = name, elapsed = ?start.elapsed(), "initialized plugin");
                return Some((host, receiver));
            }
            Ok(Err(err)) => tracing::error!(
                plugin = name,
                error = &*err,
                "plugin failed to initialize, skipping it (what it registered stays registered)"
            ),
            Err(_) => tracing::error!(
                plugin = name,
                ?timeout,
                "plugin took too long to initialize, skipping it (what it registered stays registered)"
            ),
        }
        host.handler.take();
        None
    }))
    .await;
    results.into_iter().flatten().collect()
}

struct PluginHost {
    manager: Arc<PluginManager>,
    store: Store,
    plugin: Plugin,
    handler: Option<ResourceAny>,
    name: Option<String>,
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        if let Some(handler) = self.handler.take() {
            tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(async move {
                    let _ = handler.resource_drop_async(&mut self.store).await;
                });
            });
        }
    }
}

impl PluginHost {
    fn new(manager: Arc<PluginManager>, store: Store, plugin: Plugin) -> Self {
        Self { manager, store, plugin, handler: None, name: None }
    }

    async fn initialize(&mut self) -> anyhow::Result<mpsc::Receiver<PluginRequest>> {
        let dep = self.plugin.zi_api_dependency();
        let name = dep.call_get_name(&mut self.store).await?;
        self.name = Some(name.clone());

        let lifecycle = self.plugin.zi_api_lifecycle();
        let init = lifecycle.call_initialize(&mut self.store).await?;

        let (sender, receiver) = mpsc::channel(16);

        let command = self.plugin.zi_api_command();
        self.handler = Some(command.handler().call_constructor(&mut self.store).await?);

        for cmd in init.commands {
            let Ok(name) = Word::try_from(cmd.name.as_str()) else {
                tracing::error!("invalid command name: {}", cmd.name);
                continue;
            };

            let sender = sender.clone();

            struct PluginExecutor {
                name: Word,
                sender: Sender<PluginRequest>,
            }

            impl zi::command::Executor for PluginExecutor {
                fn execute(
                    &self,
                    _client: Client,
                    range: Option<CommandRange>,
                    args: Box<[Word]>,
                    force: bool,
                ) -> BoxFuture<'static, Result<(), zi::Error>> {
                    let sender = self.sender.clone();
                    let name = self.name.clone();
                    async move {
                        let (tx, rx) = oneshot::channel();
                        sender
                            .send(PluginRequest::ExecuteCommand {
                                name: name.clone(),
                                range,
                                args,
                                force,
                                tx,
                            })
                            .await?;
                        rx.await?
                    }
                    .boxed()
                }
            }

            self.store
                .data()
                .client()
                .with(move |editor| {
                    editor.register_command(Handler::new(
                        name.clone(),
                        cmd.arity.into(),
                        cmd.opts.into(),
                        PluginExecutor { name, sender },
                    ));
                    anyhow::Ok(())
                })
                .await?;
        }

        let _id = self.manager.add(name, sender);
        Ok(receiver)
    }

    async fn run(mut self, mut receiver: mpsc::Receiver<PluginRequest>) -> anyhow::Result<()> {
        loop {
            select! {
                msg = receiver.recv() => match msg {
                    Some(req) => self.handle_request(req).await?,
                    None => break,
                }
            }
        }

        self.shutdown().await?;

        Ok(())
    }

    async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.plugin.zi_api_lifecycle().call_shutdown(&mut self.store).await?;
        if let Some(handler) = self.handler.take() {
            handler.resource_drop_async(&mut self.store).await?;
        }

        Ok(())
    }

    async fn handle_request(&mut self, req: PluginRequest) -> anyhow::Result<()> {
        match req {
            PluginRequest::ExecuteCommand { name, range, args, tx, force } => {
                let _ = range;
                let handler = self.handler.expect("handler not initialized");
                let result = self
                    .plugin
                    .zi_api_command()
                    .handler()
                    .call_exec(
                        &mut self.store,
                        handler,
                        &name,
                        &args.iter().map(|s| s.as_ref()).collect::<Box<_>>(),
                        force,
                    )
                    .await?;
                let _ = tx.send(result.map_err(|e| anyhow::anyhow!("{e}")));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use tokio::task::LocalSet;
    use zi::Editor;

    use super::*;
    use zi_plugin::Language;

    use crate::test_util::{build_fixture_plugin, build_plugin, build_source_plugin};
    use crate::wit::exports::zi::api::lifecycle::InitializeResult;

    async fn load(
        engine: &Engine,
        store: &mut Store,
        names: &[&str],
    ) -> anyhow::Result<Box<[Plugin]>> {
        let mut plugins = Vec::with_capacity(names.len());
        for name in names {
            let component = Component::new(engine, build_plugin(name)?)?;
            let linker = linker(engine)?;
            let plugin = Plugin::instantiate_async(&mut *store, &component, &linker).await?;
            plugins.push(plugin);
        }

        Ok(plugins.into_boxed_slice())
    }

    #[tokio::test]
    #[cfg_attr(test, mutants::skip)]
    async fn smoke() -> anyhow::Result<()> {
        let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
        let client = editor.client();

        let engine = engine();

        let mut store = new_store(engine, HostState::new(editor.client()));

        let local = LocalSet::new();
        local.spawn_local(editor.test_run(tasks));

        local
            .run_until(async move {
                let plugins = load(engine, &mut store, &["test"]).await?;

                for plugin in &plugins[..] {
                    let dep = plugin.zi_api_dependency();
                    assert_eq!(dep.call_get_name(&mut store).await?, "test");
                    assert_eq!(dep.call_dependencies(&mut store).await?, Vec::<String>::new());

                    let lifecycle = plugin.zi_api_lifecycle();
                    let init = lifecycle.call_initialize(&mut store).await?;

                    let rust = client
                        .with(|editor| {
                            let config = editor.language_config();
                            assert!(config.has_language_service(LanguageServiceId::new("test-ls")));
                            config.languages[&FileType::from_name("rust")]
                                .language_services
                                .to_vec()
                        })
                        .await;
                    assert_eq!(rust, [LanguageServiceId::new("test-ls")]);

                    use crate::wit::exports::zi::api::command::{Arity, Command, CommandFlags};
                    assert_eq!(
                        init,
                        InitializeResult {
                            commands: vec![
                                Command {
                                    name: "foo".into(),
                                    arity: Arity { min: 0, max: 1 },
                                    opts: CommandFlags::RANGE
                                },
                                Command {
                                    name: "stale-view".into(),
                                    arity: Arity { min: 0, max: 0 },
                                    opts: CommandFlags::empty()
                                },
                            ]
                        }
                    );
                    let handler = plugin.zi_api_command().handler();
                    let handler_resource = handler.call_constructor(&mut store).await?;
                    handler
                        .call_exec(&mut store, handler_resource, "foo", &["a"], false)
                        .await?
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    handler_resource.resource_drop_async(&mut store).await?;

                    lifecycle.call_shutdown(&mut store).await?;
                }

                Ok::<_, anyhow::Error>(())
            })
            .await
    }

    #[tokio::test]
    #[cfg_attr(test, mutants::skip)]
    async fn using_a_closed_view_traps_the_call() -> anyhow::Result<()> {
        let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
        let client = editor.client();
        let engine = engine();
        let mut store = new_store(engine, HostState::new(editor.client()));

        let local = LocalSet::new();
        local.spawn_local(editor.test_run(tasks));

        local
            .run_until(async move {
                let plugins = load(engine, &mut store, &["test"]).await?;
                let plugin = &plugins[0];
                plugin.zi_api_lifecycle().call_initialize(&mut store).await?;

                let handler = plugin.zi_api_command().handler();
                let handler_resource = handler.call_constructor(&mut store).await?;
                let err = handler
                    .call_exec(&mut store, handler_resource, "stale-view", &[], false)
                    .await
                    .expect_err("using a closed view should trap");
                assert!(format!("{err:?}").contains("no longer exists"), "{err:?}");

                assert_eq!(client.with(|editor| editor.views().count()).await, 1);
                Ok::<_, anyhow::Error>(())
            })
            .await
    }

    async fn check_fixture_plugin(lang: Language, name: &str) -> anyhow::Result<()> {
        let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
        let client = editor.client();
        let engine = engine();
        let mut store = new_store(engine, HostState::new(editor.client()));

        let local = LocalSet::new();
        local.spawn_local(editor.test_run(tasks));

        local
            .run_until(async move {
                let component = Component::new(engine, build_fixture_plugin(lang)?)?;
                let plugin =
                    Plugin::instantiate_async(&mut store, &component, &linker(engine)?).await?;
                assert_eq!(plugin.zi_api_dependency().call_get_name(&mut store).await?, name);

                use crate::wit::exports::zi::api::command::{
                    Arity, Command, CommandError, CommandFlags,
                };
                let init = plugin.zi_api_lifecycle().call_initialize(&mut store).await?;
                assert_eq!(
                    init,
                    InitializeResult {
                        commands: vec![Command {
                            name: "insert-mode".into(),
                            arity: Arity { min: 0, max: 0 },
                            opts: CommandFlags::empty(),
                        }]
                    }
                );

                let servers = client
                    .with(|editor| {
                        let config = editor.language_config();
                        assert!(config.has_language_service(LanguageServiceId::new("test-ls")));
                        config.languages[&FileType::from_name("typescript")]
                            .language_services
                            .to_vec()
                    })
                    .await;
                assert_eq!(servers, [LanguageServiceId::new("test-ls")]);

                let handler = plugin.zi_api_command().handler();
                let resource = handler.call_constructor(&mut store).await?;
                handler
                    .call_exec(&mut store, resource, "insert-mode", &[], false)
                    .await?
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                assert_eq!(client.with(|editor| editor.mode()).await, zi::Mode::Insert);

                let err = handler.call_exec(&mut store, resource, "nope", &[], false).await?;
                assert_eq!(err, Err(CommandError::Message("unexpected command: `nope`".into())));
                resource.resource_drop_async(&mut store).await?;
                Ok::<_, anyhow::Error>(())
            })
            .await
    }

    #[tokio::test]
    #[cfg_attr(test, mutants::skip)]
    async fn typescript_plugin() -> anyhow::Result<()> {
        check_fixture_plugin(Language::TypeScript, "test-ts").await
    }

    #[tokio::test]
    #[cfg_attr(test, mutants::skip)]
    async fn javascript_plugin() -> anyhow::Result<()> {
        check_fixture_plugin(Language::JavaScript, "test-js").await
    }

    #[tokio::test]
    #[cfg_attr(test, mutants::skip)]
    async fn rust_plugin() -> anyhow::Result<()> {
        check_fixture_plugin(Language::Rust, "test-rs").await
    }

    const SPIN_IN_INITIALIZE: &str = r#"
export const dependency = { getName: () => "test-spin", dependencies: () => [] };
export const lifecycle = {
  initialize() {
    for (;;) {}
  },
  shutdown() {},
};
class Handler {
  exec() {}
}
export const command = { Handler };
"#;

    #[tokio::test]
    #[cfg_attr(test, mutants::skip)]
    async fn plugins_initialize_concurrently_within_a_timeout() -> anyhow::Result<()> {
        let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
        let client = editor.client();
        let engine = engine();

        let local = LocalSet::new();
        local.spawn_local(editor.test_run(tasks));

        local
            .run_until(async move {
                let spin = Component::new(
                    engine,
                    build_source_plugin(Language::JavaScript, "test-spin", SPIN_IN_INITIALIZE)?,
                )?;
                let ok = Component::new(engine, build_fixture_plugin(Language::JavaScript)?)?;

                let manager = Arc::new(PluginManager::default());
                let linker = linker(engine)?;
                let mut hosts = vec![];
                for component in [&spin, &spin, &ok] {
                    let mut store = new_store(engine, HostState::new(client.clone()));
                    let plugin = Plugin::instantiate_async(&mut store, component, &linker).await?;
                    hosts.push(PluginHost::new(manager.clone(), store, plugin));
                }

                let timeout = Duration::from_millis(500);
                let start = Instant::now();
                let initialized = initialize_all(hosts, timeout).await;
                let elapsed = start.elapsed();

                let names = initialized
                    .into_iter()
                    .map(|(mut host, _receiver)| {
                        host.handler = None;
                        host.name.clone()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(names, [Some("test-js".to_owned())]);
                assert!(elapsed >= timeout && elapsed < 2 * timeout, "took {elapsed:?}");
                assert!(
                    client
                        .with(|editor| {
                            editor
                                .language_config()
                                .has_language_service(LanguageServiceId::new("test-ls"))
                        })
                        .await
                );
                Ok::<_, anyhow::Error>(())
            })
            .await
    }
}
