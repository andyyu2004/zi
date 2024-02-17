use std::path::Path;

use wasmtime::component::{bindgen, Component, Linker};
pub use wasmtime::{Config, Engine, Result, Store};

bindgen!({
    async: true,
    ownership: Borrowing {
        duplicate_if_necessary: true
    },
});

pub struct Loader {}

impl Loader {}

pub struct Context;

#[async_trait::async_trait]
impl PluginImports for Context {
    async fn test(&mut self) -> wasmtime::Result<String> {
        todo!()
    }
}

pub async fn load(
    engine: Engine,
    store: &mut Store<Context>,
    plugin_paths: &[impl AsRef<Path>],
) -> wasmtime::Result<Box<[Plugin]>> {
    let mut plugins = Vec::with_capacity(plugin_paths.len());
    let mut linker = Linker::new(&engine);
    for path in plugin_paths {
        let component = Component::from_file(&engine, path)?;
        Plugin::add_to_linker(&mut linker, |ctx: &mut Context| ctx)?;
        let (bindings, _) = Plugin::instantiate_async(&mut *store, &component, &linker).await?;
        plugins.push(bindings);
    }

    Ok(plugins.into_boxed_slice())
}

#[cfg(test)]
mod test {
    use wasmtime::{Config, Engine, Store};

    use crate::Context;

    #[tokio::test]
    async fn it_works() -> wasmtime::Result<()> {
        let mut config = Config::new();
        config.wasm_component_model(true).async_support(true);
        let engine = Engine::new(&config)?;
        let mut store = Store::new(&engine, Context {});
        let plugins = super::load(engine, &mut store, &["../runtime/plugins/p1.wasm"]).await?;
        for plugin in &plugins[..] {
            dbg!(plugin.call_greet(&mut store, "wer").await?);
        }
        Ok(())
    }
}
