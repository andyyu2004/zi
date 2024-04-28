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

use crate::command::{CommandHandler, CommandRange, Handler, Word};
use crate::editor::{Active, Client as EditorClient};
use crate::wit::zi::api::editor;
use crate::wit::Plugin;

pub fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true).async_support(true);
        Engine::new(&config).expect("configuration should be valid")
    })
}

pub type Store = wasmtime::Store<EditorClient>;

use crate::{dirs, Point, ViewId};

impl From<Point> for editor::Point {
    fn from(value: Point) -> Self {
        Self { line: value.line().idx() as u32, col: value.col().idx() as u32 }
    }
}

impl From<editor::Point> for Point {
    fn from(value: editor::Point) -> Self {
        Self::from((value.line, value.col))
    }
}

fn v(res: Resource<editor::View>) -> ViewId {
    ViewId::from(KeyData::from_ffi(res.rep() as u64))
}

#[async_trait::async_trait]
impl editor::HostView for EditorClient {
    async fn get_buffer(
        &mut self,
        view: Resource<editor::View>,
    ) -> wasmtime::Result<Resource<editor::Buffer>> {
        let bufnr = self.request(move |editor| editor.view(v(view)).buffer()).await;
        Ok(Resource::new_own(bufnr.data().as_ffi() as u32))
    }

    async fn get_cursor(
        &mut self,
        view: Resource<editor::View>,
    ) -> wasmtime::Result<editor::Point> {
        Ok(self.request(move |editor| editor.view(v(view)).cursor().into()).await)
    }

    async fn set_cursor(
        &mut self,
        view: Resource<editor::View>,
        pos: editor::Point,
    ) -> wasmtime::Result<()> {
        self.request(move |editor| editor.set_cursor(v(view), pos)).await;
        Ok(())
    }

    fn drop(&mut self, _rep: Resource<editor::View>) -> wasmtime::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl editor::HostBuffer for EditorClient {
    fn drop(&mut self, _rep: Resource<editor::Buffer>) -> wasmtime::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl editor::Host for EditorClient {
    async fn insert(&mut self, text: String) -> wasmtime::Result<()> {
        self.request(move |editor| editor.insert_at_cursor(&text)).await;
        Ok(())
    }

    async fn get_mode(&mut self) -> wasmtime::Result<editor::Mode> {
        Ok(self.request(|editor| editor.mode()).await)
    }

    async fn set_mode(&mut self, mode: editor::Mode) -> wasmtime::Result<()> {
        self.request(move |editor| editor.set_mode(mode)).await;
        Ok(())
    }

    async fn get_active_view(&mut self) -> wasmtime::Result<Resource<editor::View>> {
        let v = self.request(|editor| editor.view(Active).id().data().as_ffi() as u32).await;
        Ok(Resource::new_own(v))
    }
}

/// The plugin manager responsible for loading and running plugins and keeping track of their state.
/// This also provides the interface for the editor to interact with the plugins.
#[derive(Clone)]
pub struct Plugins {
    inner: Arc<Inner>,
    client: EditorClient,
}

impl Plugins {
    #[must_use]
    fn add(&self, _name: impl Into<SmolStr>, tx: Sender<PluginRequest>) -> PluginId {
        // TODO name uniqueness check?
        self.inner.plugins.write().insert(PluginState { client: PluginClient(tx) })
    }

    fn with_plugin<F, T>(&self, id: PluginId, f: F) -> wasmtime::Result<T>
    where
        F: FnOnce(&PluginState) -> T,
    {
        self.inner
            .plugins
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

#[derive(Default)]
struct Inner {
    plugins: RwLock<SlotMap<PluginId, PluginState>>,
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

impl Plugins {
    pub fn new(client: EditorClient) -> Self {
        Self { client, inner: Arc::new(Inner::default()) }
    }

    pub async fn run(self) -> wasmtime::Result<()> {
        let engine = engine();

        let components = self.load_plugin_components(engine).await?;

        let mut join_set = JoinSet::new();

        let plugin_hosts = components
            .then(|component| {
                let plugins = self.clone();
                let client = plugins.client.clone();
                let mut linker = Linker::new(engine);
                async move {
                    let component = component?;
                    let mut store = Store::new(engine, client);
                    Plugin::add_to_linker(&mut linker, |client| client)?;
                    let (plugin, _instance) =
                        Plugin::instantiate_async(&mut store, &component, &linker).await?;

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
    plugins: Plugins,
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
    fn new(plugins: Plugins, store: Store, plugin: Plugin) -> Self {
        Self { plugins, store, plugin, handler: None }
    }

    pub async fn start(mut self) -> wasmtime::Result<()> {
        let dep = self.plugin.zi_api_dependency();
        let name = dep.call_get_name(&mut self.store).await?;

        let lifecycle = self.plugin.zi_api_lifecycle();
        let init = lifecycle.call_initialize(&mut self.store).await?;

        let (tx, mut rx) = mpsc::channel(16);
        let id = self.plugins.add(name, tx);

        let command = self.plugin.zi_api_command();
        self.handler = Some(command.handler().call_constructor(&mut self.store).await?);

        for cmd in init.commands {
            let Ok(name) = Word::try_from(cmd.name.as_str()) else {
                tracing::error!("invalid command name: {}", cmd.name);
                continue;
            };

            self.store
                .data()
                .request(move |editor| {
                    editor.register_command(Handler::new(
                        name,
                        cmd.arity,
                        cmd.opts,
                        CommandHandler::Remote(id),
                    ))
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

    use super::*;
    use crate::wit::exports::zi::api::lifecycle::InitializeResult;
    use crate::Editor;

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
            let (bindings, _) = Plugin::instantiate_async(&mut *store, &component, &linker).await?;
            plugins.push(bindings);
        }

        Ok(plugins.into_boxed_slice())
    }

    #[tokio::test]
    async fn smoke() -> wasmtime::Result<()> {
        let (editor, tasks) = Editor::new(crate::Size::new(80, 24));

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
