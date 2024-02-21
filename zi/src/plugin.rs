use std::path::Path;

use wasmtime::component::{Component, Linker};
pub use wasmtime::Engine;

use crate::zi::zi::editor;

pub type Store = wasmtime::Store<Editor>;

use crate::{Editor, Plugin, PluginImports};

#[async_trait::async_trait]
impl editor::Host for Editor {
    async fn get_mode(&mut self) -> wasmtime::Result<editor::Mode> {
        Ok(self.mode())
    }

    async fn set_mode(&mut self, mode: editor::Mode) -> wasmtime::Result<()> {
        self.set_mode(mode);
        Ok(())
    }
}

#[async_trait::async_trait]
impl PluginImports for Editor {
    async fn test(&mut self) -> wasmtime::Result<String> {
        todo!()
    }
}

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
            dbg!(plugin.call_greet(&mut store, "wer").await?);
        }
        Ok(())
    }
}
