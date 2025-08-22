use std::io;

mod plugin;
mod syntax;
mod wit_conversion_impls;

pub use self::plugin::PluginManager;

mod wit {
    wasmtime::component::bindgen!({
        async: true,
        tracing: true,
        ownership: Borrowing {
            duplicate_if_necessary: true
        },
        with: {},
        additional_derives: [
            Clone,
            PartialEq,
            Eq,
        ],
    });
}

#[derive(Debug, Default)]
pub struct WasmBackend {}

impl zi::Backend for WasmBackend {
    fn new_syntax(&mut self, ft: zi::FileType) -> io::Result<Option<Box<dyn zi::Syntax>>> {
        Ok(self::syntax::Syntax::for_file_type(ft)
            .map_err(io::Error::other)?
            .map(|syntax| Box::new(syntax) as Box<dyn zi::Syntax>))
    }
}
