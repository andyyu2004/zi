#![allow(unused)]
use std::any::Any;
use std::io;
use std::path::Path;
use std::sync::OnceLock;

use futures_util::{Stream, StreamExt, TryStreamExt};
use slotmap::{Key as _, KeyData};
use tokio::select;
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tokio_stream::wrappers::ReadDirStream;
use wasmtime::component::{Component, Linker, Resource};
pub use wasmtime::Engine;

use crate::editor::Client;
use crate::wit::exports::zi::api::command;
use crate::wit::zi::api::editor;
use crate::wit::{self, Plugin};

pub fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true).async_support(true);
        Engine::new(&config).expect("configuration should be valid")
    })
}

pub type Store = wasmtime::Store<Client>;

use crate::{dirs, Editor, Point, ViewId};

impl From<Point> for editor::Position {
    fn from(value: Point) -> Self {
        Self { line: value.line().idx() as u32, col: value.col().idx() as u32 }
    }
}

impl From<editor::Position> for Point {
    fn from(value: editor::Position) -> Self {
        Self::from((value.line, value.col))
    }
}

fn v(res: Resource<editor::View>) -> ViewId {
    ViewId::from(KeyData::from_ffi(res.rep() as u64))
}

#[async_trait::async_trait]
impl editor::HostView for Client {
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
    ) -> wasmtime::Result<editor::Position> {
        Ok(self.request(move |editor| editor.view(v(view)).cursor().into()).await)
    }

    async fn set_cursor(
        &mut self,
        view: Resource<editor::View>,
        pos: editor::Position,
    ) -> wasmtime::Result<()> {
        self.request(move |editor| editor.set_cursor(v(view), pos)).await;
        Ok(())
    }

    fn drop(&mut self, _rep: Resource<editor::View>) -> wasmtime::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl editor::HostBuffer for Client {
    fn drop(&mut self, _rep: Resource<editor::Buffer>) -> wasmtime::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl editor::Host for Client {
    async fn insert(&mut self, text: String) -> wasmtime::Result<()> {
        self.request(move |editor| editor.insert(&text)).await;
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
        let v = self.request(|editor| editor.active_view().id().data().as_ffi() as u32).await;
        Ok(Resource::new_own(v))
    }

    async fn create_command(&mut self, cmd: String) -> wasmtime::Result<()> {
        Ok(())
    }
}

pub struct Plugins {
    client: Client,
}

impl Plugins {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn run(self) -> wasmtime::Result<()> {
        let engine = engine();

        let components = self.load_plugin_components(engine).await?;

        let mut join_set = JoinSet::new();

        let plugin_hosts = components
            .then(|component| {
                let client = self.client.clone();
                let mut linker = Linker::new(engine);
                async move {
                    let component = component?;
                    let mut store = Store::new(engine, client);
                    Plugin::add_to_linker(&mut linker, |client| client)?;
                    let (plugin, _instance) =
                        Plugin::instantiate_async(&mut store, &component, &linker).await?;

                    Ok::<_, wasmtime::Error>(PluginHost { plugin, store })
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
    store: Store,
    plugin: Plugin,
}

impl PluginHost {
    pub async fn start(mut self) -> wasmtime::Result<()> {
        self.plugin.call_initialize(&mut self.store).await?;

        loop {
            select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
            }
        }
        Ok(())
    }

    async fn initialize(&mut self) -> wasmtime::Result<()> {
        self.plugin.call_initialize(&mut self.store).await
    }
}

#[cfg(test)]
mod test {
    use futures_util::{Stream, StreamExt};

    use super::*;

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
        let (mut editor, tasks) = Editor::new(crate::Size::new(80, 24));

        let engine = engine();

        let mut store = Store::new(engine, editor.client());

        tokio::spawn(editor.test_run(tasks));

        let plugins = load(engine, &mut store, &["../runtime/plugins/plugin_test.wasm"]).await?;

        for plugin in &plugins[..] {
            assert_eq!(plugin.call_get_name(&mut store).await?, "test");
            assert!(plugin.call_dependencies(&mut store).await?.is_empty());
            plugin.call_initialize(&mut store).await?;

            let handler = plugin.zi_api_command().handler();
            let handler_resource = handler.call_constructor(&mut store).await?;
            assert_eq!(handler.call_exec(&mut store, handler_resource, "wer", &["a"]).await?, 42);
            handler_resource.resource_drop_async(&mut store).await?;
        }

        Ok(())
    }
}
