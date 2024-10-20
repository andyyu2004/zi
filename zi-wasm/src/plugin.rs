use std::io;
use std::sync::{Arc, OnceLock};

use futures_util::{Stream, StreamExt, TryStreamExt};
use parking_lot::RwLock;
use slotmap::{Key as _, KeyData, SlotMap};
use smol_str::SmolStr;
use tokio::select;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tokio_stream::wrappers::ReadDirStream;
use wasmtime::component::{Component, Linker, Resource, ResourceAny};
pub use wasmtime::Engine;
use zi::command::{CommandRange, Word};
use zi::{dirs, Active, Client, Point, ViewId};

use crate::wit::zi::api;
use crate::wit::Plugin;

pub fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true).async_support(true);
        Engine::new(&config).expect("configuration should be valid")
    })
}

pub type Store = wasmtime::Store<Client>;

fn v(res: Resource<api::editor::View>) -> ViewId {
    ViewId::from(KeyData::from_ffi(res.rep() as u64))
}

#[async_trait::async_trait]
impl api::editor::HostView for Client {
    async fn get_buffer(
        &mut self,
        view: Resource<api::editor::View>,
    ) -> Resource<api::editor::Buffer> {
        let bufnr = self.with(move |editor| editor.view(v(view)).buffer()).await;
        Resource::new_own(bufnr.data().as_ffi() as u32)
    }

    async fn get_cursor(&mut self, view: Resource<api::editor::View>) -> api::editor::Point {
        self.with(move |editor| editor.view(v(view)).cursor().into()).await
    }

    async fn set_cursor(&mut self, view: Resource<api::editor::View>, pos: api::editor::Point) {
        self.with(move |editor| editor.set_cursor(v(view), Point::from(pos))).await
    }

    async fn drop(&mut self, _rep: Resource<api::editor::View>) -> wasmtime::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl api::editor::HostBuffer for Client {
    async fn drop(&mut self, _rep: Resource<api::editor::Buffer>) -> wasmtime::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl api::editor::Host for Client {
    async fn insert(&mut self, text: String) -> Result<(), api::editor::EditError> {
        Ok(self.with(move |editor| editor.insert(Active, &text)).await?)
    }

    async fn get_mode(&mut self) -> api::editor::Mode {
        self.with(|editor| editor.mode()).await.into()
    }

    async fn set_mode(&mut self, mode: api::editor::Mode) {
        self.with(move |editor| editor.set_mode(mode.into())).await
    }

    async fn get_active_view(&mut self) -> Resource<api::editor::View> {
        let v = self.with(|editor| editor.view(Active).id().data().as_ffi() as u32).await;
        Resource::new_own(v)
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

    fn with_plugin<F, T>(&self, id: PluginId, f: F) -> wasmtime::Result<T>
    where
        F: FnOnce(&PluginState) -> T,
    {
        self.plugins
            .read()
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("plugin not found: {id:?}",))
            .map(f)
    }

    fn plugin_client(&self, id: PluginId) -> wasmtime::Result<PluginClient> {
        self.with_plugin(id, |state| state.client.clone())
    }

    pub async fn execute(
        &self,
        id: PluginId,
        name: Word,
        range: Option<CommandRange>,
        args: Box<[String]>,
    ) -> wasmtime::Result<()> {
        let client = self.plugin_client(id)?;
        client.execute(name, range, args).await
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
        args: Box<[String]>,
    ) -> wasmtime::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.0.send(PluginRequest::ExecuteCommand { name, range, args, tx }).await?;
        rx.await?
    }
}

struct PluginState {
    client: PluginClient,
}

type Responder<T> = oneshot::Sender<wasmtime::Result<T>>;

enum PluginRequest {
    ExecuteCommand {
        name: Word,
        range: Option<CommandRange>,
        args: Box<[String]>,
        tx: Responder<()>,
    },
}

#[async_trait::async_trait]
impl zi::plugin::PluginManager for PluginManager {
    fn name(&self) -> &'static str {
        "wasm"
    }

    async fn start(self: Arc<Self>, client: Client) -> wasmtime::Result<()> {
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
                    let mut store = Store::new(engine, client);
                    Plugin::add_to_linker(&mut linker, |client| client)?;
                    let plugin = Plugin::instantiate_async(&mut store, &component, &linker).await?;

                    Ok::<_, wasmtime::Error>(PluginHost::new(plugins, store, plugin))
                }
            })
            .filter_map(|res| async {
                match res {
                    Ok(plugin) => Some(plugin),
                    Err(err) => {
                        tracing::error!("error loading plugin: {err} (skipping)");
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
                Ok(Err(err)) => tracing::error!("error running plugin: {err}"),
                Err(err) => tracing::error!("error joining plugin: {err}"),
            }
        }

        Ok(())
    }
}

impl PluginManager {
    async fn load_plugin_components(
        &self,
        engine: &'static Engine,
    ) -> io::Result<impl Stream<Item = wasmtime::Result<Component>>> {
        let entries = tokio::fs::read_dir(dirs::plugin()).await?;
        let stream = ReadDirStream::new(entries);
        Ok(stream.map_err(Into::into).try_filter_map(move |entry| async move {
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

    pub async fn start(mut self) -> wasmtime::Result<()> {
        let dep = self.plugin.zi_api_dependency();
        let name = dep.call_get_name(&mut self.store).await?;

        let lifecycle = self.plugin.zi_api_lifecycle();
        let init = lifecycle.call_initialize(&mut self.store).await?;

        let (tx, mut rx) = mpsc::channel(16);
        let _id = self.manager.add(name, tx);

        let command = self.plugin.zi_api_command();
        self.handler = Some(command.handler().call_constructor(&mut self.store).await?);

        for cmd in init.commands {
            let Ok(_name) = Word::try_from(cmd.name.as_str()) else {
                tracing::error!("invalid command name: {}", cmd.name);
                continue;
            };

            self.store
                .data()
                .with(move |_editor| {
                    todo!();
                    // editor.register_command(Handler::new(
                    //     name,
                    //     cmd.arity.into(),
                    //     cmd.opts.into(),
                    //     CommandHandler::Remote(id),
                    // ))
                })
                .await;
        }

        loop {
            select! {
                msg = rx.recv() => match msg {
                    Some(req) => self.handle_request(req).await?,
                    None => break,
                }
            }
        }

        self.shutdown().await?;

        Ok(())
    }

    async fn shutdown(&mut self) -> wasmtime::Result<()> {
        self.plugin.zi_api_lifecycle().call_shutdown(&mut self.store).await?;
        if let Some(handler) = self.handler.take() {
            handler.resource_drop_async(&mut self.store).await?;
        }

        Ok(())
    }

    async fn handle_request(&mut self, req: PluginRequest) -> wasmtime::Result<()> {
        match req {
            PluginRequest::ExecuteCommand { name, range, args, tx } => {
                let _ = range;
                let handler = self.handler.expect("handler not initialized");
                self.plugin
                    .zi_api_command()
                    .handler()
                    .call_exec(
                        &mut self.store,
                        handler,
                        &name,
                        &args.iter().map(|s| s.as_ref()).collect::<Box<_>>(),
                    )
                    .await?;
                let _ = tx.send(Ok(()));
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
    ) -> wasmtime::Result<Box<[Plugin]>> {
        let mut plugins = Vec::with_capacity(plugin_paths.len());
        let mut linker = Linker::new(engine);
        for path in plugin_paths {
            let component = Component::from_file(engine, path)?;
            Plugin::add_to_linker(&mut linker, |client| client)?;
            let plugin = Plugin::instantiate_async(&mut *store, &component, &linker).await?;
            plugins.push(plugin);
        }

        Ok(plugins.into_boxed_slice())
    }

    #[tokio::test]
    #[cfg_attr(test, mutants::skip)]
    async fn smoke() -> wasmtime::Result<()> {
        let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));

        let engine = engine();

        let mut store = Store::new(engine, editor.client());

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
                    handler.call_exec(&mut store, handler_resource, "foo", &["a"]).await?;
                    handler_resource.resource_drop_async(&mut store).await?;

                    lifecycle.call_shutdown(&mut store).await?;
                }

                Ok::<_, anyhow::Error>(())
            })
            .await
    }
}
