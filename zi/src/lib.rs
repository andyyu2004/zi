use std::io;

pub use zi_editor::*;
mod syntax;

mod wit {
    wasmtime::component::bindgen!({
        async: true,
        tracing: true,
        ownership: Borrowing {
            duplicate_if_necessary: true
        },
        with: {}
    });
}

#[derive(Debug, Default)]
pub struct WasmBackend {}

impl Backend for WasmBackend {
    fn new_syntax(&mut self, ft: FileType) -> io::Result<Option<Box<dyn Syntax>>> {
        Ok(self::syntax::Syntax::for_file_type(ft)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
            .map(|syntax| Box::new(syntax) as Box<dyn Syntax>))
    }
}
