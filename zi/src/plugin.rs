#![allow(unused)]
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

impl From<editor::Position> for Position {
    fn from(value: editor::Position) -> Self {
        Self::from((value.line, value.col))
    }
}

fn v(res: Resource<editor::View>) -> ViewId {
    ViewId::from(KeyData::from_ffi(res.rep() as u64))
}

impl editor::HostView for Editor {
    fn get_buffer(
        &mut self,
        view: Resource<editor::View>,
    ) -> wasmtime::Result<Resource<editor::Buffer>> {
        Ok(Resource::new_own(self.view(v(view)).buffer().data().as_ffi() as u32))
    }

    fn get_cursor(&mut self, view: Resource<editor::View>) -> wasmtime::Result<editor::Position> {
        Ok(self.view(v(view)).cursor().into())
    }

    fn set_cursor(
        &mut self,
        view: Resource<editor::View>,
        pos: editor::Position,
    ) -> wasmtime::Result<()> {
        self.set_cursor(v(view), pos);
        Ok(())
    }

    fn drop(&mut self, _rep: Resource<editor::View>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl editor::HostBuffer for Editor {
    fn drop(&mut self, _rep: Resource<editor::Buffer>) -> wasmtime::Result<()> {
        Ok(())
    }
}

impl editor::Host for Editor {
    fn get_mode(&mut self) -> wasmtime::Result<editor::Mode> {
        Ok(self.mode())
    }

    fn set_mode(&mut self, mode: editor::Mode) -> wasmtime::Result<()> {
        self.set_mode(mode);
        Ok(())
    }

    fn get_active_view(&mut self) -> wasmtime::Result<Resource<editor::View>> {
        Ok(Resource::new_own(self.active_view().id().data().as_ffi() as u32))
    }

    fn insert(&mut self, text: String) -> wasmtime::Result<()> {
        self.insert(&text);
        Ok(())
    }
}

pub fn load(
    engine: Engine,
    store: &mut Store,
    plugin_paths: &[impl AsRef<Path>],
) -> wasmtime::Result<Box<[Plugin]>> {
    let mut plugins = Vec::with_capacity(plugin_paths.len());
    let mut linker = Linker::new(&engine);
    for path in plugin_paths {
        let component = Component::from_file(&engine, path)?;
        Plugin::add_to_linker(&mut linker, |ctx| ctx)?;
        let (bindings, _) = Plugin::instantiate(&mut *store, &component, &linker)?;
        plugins.push(bindings);
    }

    Ok(plugins.into_boxed_slice())
}

#[cfg(test)]
mod test {
    use wasmtime::{Config, Engine, Store};

    use crate::Editor;

    fn it_works() -> wasmtime::Result<()> {
        let mut config = Config::new();
        config.wasm_component_model(true).async_support(true);
        let engine = Engine::new(&config)?;
        let (editor, _) = Editor::new(crate::Size::new(80, 24));
        let mut store = Store::new(&engine, editor);
        let plugins = super::load(engine, &mut store, &["../runtime/plugins/p1.wasm"])?;
        for plugin in &plugins[..] {
            plugin.call_initialize(&mut store)?;
        }
        Ok(())
    }
}
