use api::command::{self, CommandError};
use api::dependency;
use api::lifecycle::{self, InitializeResult};
use bindings::zi::api::editor;

use self::bindings::exports::zi::api;

struct Component;

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../zi-wasm/wit/zi.wit",
        additional_derives: [PartialEq, Eq],
        ownership: Borrowing { duplicate_if_necessary: true },
    });
}

impl command::Guest for Component {
    type Handler = CommandHandler;
}

struct CommandHandler;

impl command::GuestHandler for CommandHandler {
    fn new() -> Self {
        Self
    }

    fn exec(&self, cmd: String, _args: Vec<String>, _force: bool) -> Result<(), CommandError> {
        Err(CommandError::Message(format!("unexpected command: `{cmd}`")))
    }
}

impl lifecycle::Guest for Component {
    fn initialize() -> InitializeResult {
        editor::register_language("rust", &["rust-analyzer"]);
        editor::register_language("fsharp", &["fsautocomplete"]);
        editor::register_language("text", &[]);
        editor::register_language("toml", &[]);
        editor::register_language("json", &[]);
        editor::register_language("yaml", &[]);
        editor::register_language("python", &["pyright"]);
        editor::register_language("haskell", &["hls"]);
        editor::register_language("go", &["gopls"]);
        editor::register_language("gqlt", &["gqlt"]);
        editor::register_language("c", &["clangd"]);
        editor::register_language("zig", &["zls"]);
        editor::register_language("javascript", &["typescript-language-server"]);
        editor::register_language("typescript", &["typescript-language-server"]);

        editor::register_language_server("zls", "zls", &[]);
        editor::register_language_server("fsautocomplete", "fsautocomplete", &[]);
        editor::register_language_server("rust-analyzer", "lspmux", &["client"]);
        editor::register_language_server("gopls", "gopls", &[]);
        editor::register_language_server("gqlt", "gqlt", &[]);
        editor::register_language_server("clangd", "clangd", &[]);
        editor::register_language_server("pyright", "pyright-langserver", &["--stdio"]);
        editor::register_language_server("hls", "haskell-language-server-wrapper", &["--lsp"]);
        editor::register_language_server(
            "typescript-language-server",
            "typescript-language-server",
            &["--stdio"],
        );

        InitializeResult { commands: vec![] }
    }

    fn shutdown() {}
}

impl dependency::Guest for Component {
    fn get_name() -> String {
        "config".into()
    }

    fn dependencies() -> Vec<String> {
        vec![]
    }
}

bindings::export!(Component with_types_in bindings);
