use std::path::Path;

use slotmap::{Key as _, KeyData};
use wasmtime::component::{Component, Linker, Resource};
pub use wasmtime::Engine;

use crate::zi::api::editor;

pub type Store = wasmtime::Store<Editor>;

use crate::{Editor, Plugin, Position, ViewId};

impl From<Position> for editor::Position {
    fn from(value: Position) -> Self {
        Self { line: value.line().idx() as u32, col: value.col().idx() as u32 }
    }
}

#[async_trait::async_trait]
impl editor::HostView for Editor {
    async fn get_buffer(
        &mut self,
        view: Resource<editor::View>,
    ) -> wasmtime::Result<Resource<editor::Buffer>> {
        Ok(Resource::new_own(
            self.view(ViewId::from(KeyData::from_ffi(view.rep() as u64))).buffer().data().as_ffi()
                as u32,
        ))
    }

    async fn get_cursor(
        &mut self,
        view: Resource<editor::View>,
    ) -> wasmtime::Result<editor::Position> {
        Ok(self.view(ViewId::from(KeyData::from_ffi(view.rep() as u64))).cursor().into())
    }

    fn drop(&mut self, _rep: Resource<editor::View>) -> wasmtime::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl editor::HostBuffer for Editor {
    fn drop(&mut self, _rep: Resource<editor::Buffer>) -> wasmtime::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl editor::Host for Editor {
    async fn get_mode(&mut self) -> wasmtime::Result<editor::Mode> {
        Ok(self.mode())
    }

    async fn set_mode(&mut self, mode: editor::Mode) -> wasmtime::Result<()> {
        self.set_mode(mode);
        Ok(())
    }

    async fn get_active_view(&mut self) -> wasmtime::Result<Resource<editor::View>> {
        Ok(Resource::new_own(self.active_view().id().data().as_ffi() as u32))
    }

    // async fn get_view(&mut self, id: editor::ViewId) -> wasmtime::Result<Resource<editor::View>> {
    //     Ok(Resource::new_borrow(id))
    // }
}

// #[async_trait::async_trait]
// impl PluginImports for Editor {
//     async fn test(&mut self) -> wasmtime::Result<String> {
//         todo!()
//     }
// }

pub async fn load(
    engine: Engine,
    store: &mut Store,
    plugin_paths: &[impl AsRef<Path>],
) -> wasmtime::Result<Box<[Plugin]>> {
    let mut plugins = Vec::with_capacity(plugin_paths.len());
    let mut linker = Linker::new(&engine);
    for path in plugin_paths {
        let component = Component::from_file(&engine, path)?;
        Plugin::add_to_linker(&mut linker, |ctx| ctx)?;
        let (bindings, _) = Plugin::instantiate_async(&mut *store, &component, &linker).await?;
        plugins.push(bindings);
    }

    Ok(plugins.into_boxed_slice())
}

#[cfg(test)]
mod test {
    use wasmtime::{Config, Engine, Store};

    use crate::Editor;

    #[tokio::test]
    async fn it_works() -> wasmtime::Result<()> {
        let mut config = Config::new();
        config.wasm_component_model(true).async_support(true);
        let engine = Engine::new(&config)?;
        let mut store = Store::new(&engine, Editor::default());
        let plugins = super::load(engine, &mut store, &["../runtime/plugins/p1.wasm"]).await?;
        for plugin in &plugins[..] {
            plugin.call_initialize(&mut store).await?;
        }
        Ok(())
    }
}
