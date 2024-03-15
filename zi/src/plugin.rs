#![allow(unused)]
use std::any::Any;
use std::path::Path;
use std::sync::OnceLock;

use slotmap::{Key as _, KeyData};
use tokio::sync::oneshot;
use wasmtime::component::{Component, Linker, Resource};
pub use wasmtime::Engine;

use crate::editor::Client;
use crate::wit::zi::api::editor;
use crate::wit::{self, CommandHandler, Plugin};

pub fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = wasmtime::Config::new();
        config.wasm_component_model(true).async_support(true);
        Engine::new(&config).expect("configuration should be valid")
    })
}

pub type Store = wasmtime::Store<Client>;

use crate::{Editor, Point, ViewId};

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

impl editor::HostBuffer for Client {
    fn drop(&mut self, _rep: Resource<editor::Buffer>) -> wasmtime::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl wit::HostCommandHandler for Client {
    async fn new(&mut self) -> wasmtime::Result<Resource<CommandHandler>> {
        panic!("guest provides command handler resource")
    }

    async fn exec(
        &mut self,
        _rep: Resource<CommandHandler>,
        cmd: String,
        args: Vec<String>,
    ) -> wasmtime::Result<()> {
        panic!("guest provides command handler resource")
    }

    fn drop(&mut self, _rep: Resource<CommandHandler>) -> wasmtime::Result<()> {
        panic!("guest provides command handler resource")
    }
}

impl wit::PluginImports for Client {}

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

pub async fn load(
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

#[cfg(test)]
mod test {
    use futures_util::{Stream, StreamExt};
    use wasmtime::{Config, Engine, Store};

    use super::engine;
    use crate::Editor;

    #[tokio::test]
    async fn it_works() -> wasmtime::Result<()> {
        let (mut editor, tasks) = Editor::new(crate::Size::new(80, 24));

        let engine = engine();

        let mut store = Store::new(engine, editor.client());

        tokio::spawn(editor.test_run(tasks));

        let plugins = super::load(engine, &mut store, &["../runtime/plugins/example.wasm"]).await?;

        for plugin in &plugins[..] {
            assert_eq!(plugin.call_get_name(&mut store).await?, "example");
            assert!(plugin.call_dependencies(&mut store).await?.is_empty());
            plugin.call_initialize(&mut store).await?;
            let handler = plugin.call_handler(&mut store).await?;
        }

        Ok(())
    }
}
