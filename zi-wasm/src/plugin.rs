use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use futures_util::future::BoxFuture;
use futures_util::{FutureExt, Stream, StreamExt, TryStreamExt};
use parking_lot::RwLock;
use slotmap::SlotMap;
use smol_str::SmolStr;
use tokio::select;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tokio_stream::wrappers::ReadDirStream;
pub use wasmtime::Engine;
use wasmtime::component::{Component, Linker, Resource, ResourceAny};
use zi::command::{self, CommandRange, Handler, Word};
use zi::{Active, BufferId, Client, Point, ViewId, dirs};

use crate::wit::Plugin;
use crate::wit::zi::api;

pub fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true);
        Engine::new(&config).expect("configuration should be valid")
    })
}

pub struct HostState {
    client: Client,
    // Need some mappings from wasm ui32 wasm resource rep to slotmap u64 keys.
    views: HashMap<u32, ViewId>,
    buffers: HashMap<u32, BufferId>,
    next_rep: AtomicU32,
}

impl HostState {
    fn new(client: Client) -> Self {
        Self { client, views: HashMap::new(), buffers: HashMap::new(), next_rep: AtomicU32::new(1) }
    }

    fn client(&self) -> &Client {
        &self.client
    }

    fn push_view(&mut self, id: ViewId) -> Resource<api::editor::View> {
        let rep = self.next_rep.fetch_add(1, Ordering::Relaxed);
        self.views.insert(rep, id);
        Resource::new_own(rep)
    }

    fn get_view(&self, res: &Resource<api::editor::View>) -> ViewId {
        self.views[&res.rep()]
    }

    fn push_buffer(&mut self, id: BufferId) -> Resource<api::editor::Buffer> {
        let rep = self.next_rep.fetch_add(1, Ordering::Relaxed);
        self.buffers.insert(rep, id);
        Resource::new_own(rep)
    }
}

pub type Store = wasmtime::Store<HostState>;

impl api::editor::HostView for HostState {
    async fn get_buffer(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> Resource<api::editor::Buffer> {
        let view_id = self.get_view(&view);
        let buf_id = self.client.with(move |editor| editor.view(view_id).buffer()).await;
        self.push_buffer(buf_id)
    }

    async fn get_cursor(&mut self, view: Resource<api::editor::View>) -> api::editor::Point {
        let view_id = self.get_view(&view);
        self.client.with(move |editor| editor.view(view_id).cursor().into()).await
    }

    async fn set_cursor(&mut self, view: Resource<api::editor::View>, pos: api::editor::Point) {
        let view_id = self.get_view(&view);
        self.client.with(move |editor| editor.set_cursor(view_id, Point::from(pos))).await
    }

    async fn close(&mut self, view: Resource<api::editor::View>) {
        let view_id = self.get_view(&view);
        command::close_view(&self.client, view_id).await
    }

    async fn save(&mut self, view: Resource<api::editor::View>, force: bool) -> Result<(), String> {
        let view_id = self.get_view(&view);
        command::save(&self.client, view_id, force).await.map_err(|e| e.to_string())
    }

    async fn inspect(&mut self, view: Resource<api::editor::View>) {
        let view_id = self.get_view(&view);
        command::inspect(&self.client, view_id).await
    }

    async fn insert(
        &mut self,
        view: Resource<api::editor::View>,
        text: String,
    ) -> Result<(), api::editor::EditError> {
        let view_id = self.get_view(&view);
        Ok(self.client.with(move |editor| editor.insert(view_id, &text)).await?)
    }

    async fn delete_char(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> Result<(), api::editor::EditError> {
        let view_id = self.get_view(&view);
        Ok(self.client.with(move |editor| editor.delete_char(view_id)).await?)
    }

    async fn paste_after(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> Result<(), api::editor::EditError> {
        let view_id = self.get_view(&view);
        Ok(self.client.with(move |editor| editor.paste_after(view_id)).await?)
    }

    async fn undo(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> Result<bool, api::editor::EditError> {
        let view_id = self.get_view(&view);
        let buf_id = self.client.with(move |editor| editor.view(view_id).buffer()).await;
        Ok(self.client.with(move |editor| editor.undo(buf_id)).await?)
    }

    async fn redo(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> Result<bool, api::editor::EditError> {
        let view_id = self.get_view(&view);
        let buf_id = self.client.with(move |editor| editor.view(view_id).buffer()).await;
        Ok(self.client.with(move |editor| editor.redo(buf_id)).await?)
    }

    async fn scroll(
        &mut self,
        view: Resource<api::editor::View>,
        direction: api::editor::Direction,
        amount: u32,
    ) {
        let view_id = self.get_view(&view);
        let dir = zi::Direction::from(direction);
        self.client
            .with(move |editor| editor.scroll(view_id, dir, amount as usize))
            .await
    }

    async fn align(
        &mut self,
        view: Resource<api::editor::View>,
        alignment: api::editor::VerticalAlignment,
    ) {
        let view_id = self.get_view(&view);
        let align = zi::VerticalAlignment::from(alignment);
        self.client
            .with(move |editor| editor.align_view(view_id, align))
            .await
    }

    async fn focus(&mut self, view: Resource<api::editor::View>) {
        let view_id = self.get_view(&view);
        self.client.with(move |editor| editor.focus(view_id)).await
    }

    async fn drop(&mut self, view: Resource<api::editor::View>) -> wasmtime::Result<()> {
        self.views.remove(&view.rep());
        Ok(())
    }
}

impl api::editor::HostBuffer for HostState {
    async fn get_file_path(
        &mut self,
        buf: Resource<api::editor::Buffer>,
    ) -> Option<String> {
        let buf_id = self.buffers[&buf.rep()];
        self.client
            .with(move |editor| {
                editor.buffer(buf_id).file_path().map(|p| p.to_string_lossy().into_owned())
            })
            .await
    }

    async fn get_flags(
        &mut self,
        buf: Resource<api::editor::Buffer>,
    ) -> api::editor::BufferFlags {
        let buf_id = self.buffers[&buf.rep()];
        self.client
            .with(move |editor| editor.buffer(buf_id).flags().into())
            .await
    }

    async fn len_lines(&mut self, buf: Resource<api::editor::Buffer>) -> u32 {
        let buf_id = self.buffers[&buf.rep()];
        self.client
            .with(move |editor| editor.buffer(buf_id).text().len_lines() as u32)
            .await
    }

    async fn drop(&mut self, buf: Resource<api::editor::Buffer>) -> wasmtime::Result<()> {
        self.buffers.remove(&buf.rep());
        Ok(())
    }
}

impl api::editor::Host for HostState {
    async fn insert(&mut self, text: String) -> Result<(), api::editor::EditError> {
        Ok(self.client.with(move |editor| editor.insert(Active, &text)).await?)
    }

    async fn get_mode(&mut self) -> api::editor::Mode {
        self.client.with(|editor| editor.mode()).await.into()
    }

    async fn set_mode(&mut self, mode: api::editor::Mode) {
        self.client.with(move |editor| editor.set_mode(mode.into())).await
    }

    async fn get_active_view(&mut self) -> Resource<api::editor::View> {
        let view_id = self.client.with(|editor| editor.view(Active).id()).await;
        self.push_view(view_id)
    }

    async fn save_all(&mut self, force: bool) -> Result<(), String> {
        command::save_all(&self.client, force).await.map_err(|e| e.to_string())
    }

    async fn reload(&mut self) -> Result<(), String> {
        command::reload(&self.client).await.map_err(|e| e.to_string())
    }

    async fn open_jump_list(&mut self) {
        self.client
            .with(|editor| {
                editor.open_jump_list(Active);
            })
            .await
    }

    async fn open_file_explorer(&mut self, path: String) {
        self.client.with(move |editor| editor.open_file_explorer(path)).await
    }

    async fn set_option(&mut self, key: String, value: String) -> Result<(), String> {
        self.client
            .with(move |editor| command::set_option(editor, &key, &value))
            .await
            .map_err(|e| e.to_string())
    }
}

/// The plugin manager responsible for loading and running wasm plugins and keeping track of their state.
/// This also provides the interface for the editor to interact with the plugins.
#[derive(Default)]
pub struct PluginManager {
    plugins: RwLock<SlotMap<PluginId, PluginState>>,
}

impl PluginManager {
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
            .ok_or_else(|| anyhow::anyhow!("plugin not found: {id:?}",))
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

        let components = self.load_plugin_components(engine).await?;

        let mut join_set = JoinSet::new();

        let plugin_hosts = components
            .then(|component| {
                let plugins = self.clone();
                let client = client.clone();
                let mut linker = Linker::new(engine);
                async move {
                    let component = component?;
                    let mut store = Store::new(engine, HostState::new(client));
                    Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(
                        &mut linker,
                        |state| state,
                    )?;
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

        for host in plugin_hosts.into_vec() {
            join_set.spawn(host.start());
        }

        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(Ok(())) => {}
                Ok(Err(err)) => tracing::error!(error = &*err, "error running plugin"),
                Err(err) => {
                    tracing::error!(error = &err as &dyn std::error::Error, "error joining plugin")
                }
            }
        }

        Ok(())
    }
}

impl PluginManager {
    async fn load_plugin_components(
        &self,
        engine: &'static Engine,
    ) -> io::Result<impl Stream<Item = anyhow::Result<Component>>> {
        let dirs = futures_util::stream::iter(dirs::plugin());
        let entries = dirs
            .then(|dir| async move {
                let entries = tokio::fs::read_dir(dir).await?;
                Ok::<_, io::Error>(ReadDirStream::new(entries))
            })
            .try_flatten();

        Ok(entries.map_err(Into::into).try_filter_map(move |entry| async move {
            if !entry.file_type().await?.is_file() {
                return Ok(None);
            }

            let component = Component::from_file(engine, entry.path())?;
            Ok(Some(component))
        }))
    }
}

struct PluginHost {
    manager: Arc<PluginManager>,
    store: Store,
    plugin: Plugin,
    handler: Option<ResourceAny>,
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
        Self { manager, store, plugin, handler: None }
    }

    pub async fn start(mut self) -> anyhow::Result<()> {
        let dep = self.plugin.zi_api_dependency();
        let name = dep.call_get_name(&mut self.store).await?;

        let lifecycle = self.plugin.zi_api_lifecycle();
        let init = lifecycle.call_initialize(&mut self.store).await?;

        let (sender, mut receiver) = mpsc::channel(16);
        let _id = self.manager.add(name, sender.clone());

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
    use std::path::Path;

    use tokio::task::LocalSet;
    use zi::Editor;

    use super::*;
    use crate::wit::exports::zi::api::lifecycle::InitializeResult;

    async fn load(
        engine: &Engine,
        store: &mut Store,
        plugin_paths: &[impl AsRef<Path>],
    ) -> anyhow::Result<Box<[Plugin]>> {
        let mut plugins = Vec::with_capacity(plugin_paths.len());
        let mut linker = Linker::new(engine);
        for path in plugin_paths {
            let component = Component::from_file(engine, path)?;
            Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| {
                state
            })?;
            let plugin = Plugin::instantiate_async(&mut *store, &component, &linker).await?;
            plugins.push(plugin);
        }

        Ok(plugins.into_boxed_slice())
    }

    #[tokio::test]
    #[cfg_attr(test, mutants::skip)]
    async fn smoke() -> anyhow::Result<()> {
        let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));

        let engine = engine();

        let mut store = Store::new(engine, HostState::new(editor.client()));

        let local = LocalSet::new();
        local.spawn_local(editor.test_run(tasks));

        local
            .run_until(async move {
                let plugins =
                    load(engine, &mut store, &["../runtime/plugins/plugin_test.wasm"]).await?;

                for plugin in &plugins[..] {
                    let dep = plugin.zi_api_dependency();
                    assert_eq!(dep.call_get_name(&mut store).await?, "test");
                    assert!(dep.call_dependencies(&mut store).await?.is_empty());

                    let lifecycle = plugin.zi_api_lifecycle();
                    let init = lifecycle.call_initialize(&mut store).await?;

                    use crate::wit::exports::zi::api::command::{Arity, Command, CommandFlags};
                    assert_eq!(
                        init,
                        InitializeResult {
                            commands: vec![Command {
                                name: "foo".into(),
                                arity: Arity { min: 0, max: 1 },
                                opts: CommandFlags::RANGE
                            }]
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
}
