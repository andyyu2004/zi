use bindings::exports::zi::api::command::{self, Arity, Command, CommandError, CommandFlags};
use bindings::exports::zi::api::{dependency, lifecycle};
use bindings::zi::api::editor::{self, LanguageServer, Mode};

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({ path: "wit", world: "plugin" });
}

struct Plugin;

impl dependency::Guest for Plugin {
    fn get_name() -> String {
        "test-rs".into()
    }

    fn dependencies() -> Vec<String> {
        vec![]
    }
}

impl lifecycle::Guest for Plugin {
    fn initialize() -> lifecycle::InitializeResult {
        let servers = ["test-ls".to_owned()];
        assert_eq!(
            editor::add_language("typescript", &servers),
            Err("language server `test-ls` for language `typescript` is not defined".into())
        );
        editor::add_language_server(
            "test-ls",
            &LanguageServer { command: "test-ls".into(), args: vec!["--stdio".into()] },
        );
        editor::add_language("typescript", &servers).unwrap();

        lifecycle::InitializeResult {
            commands: vec![Command {
                name: "insert-mode".into(),
                arity: Arity { min: 0, max: 0 },
                opts: CommandFlags::empty(),
            }],
        }
    }

    fn shutdown() {}
}

impl command::Guest for Plugin {
    type Handler = Handler;
}

struct Handler;

impl command::GuestHandler for Handler {
    fn new() -> Self {
        Self
    }

    fn exec(&self, cmd: String, _args: Vec<String>, _force: bool) -> Result<(), CommandError> {
        match cmd.as_str() {
            "insert-mode" => {
                editor::set_mode(Mode::Insert);
                assert!(matches!(editor::get_mode(), Mode::Insert));
                Ok(())
            }
            _ => Err(CommandError::Message(format!("unexpected command: `{cmd}`"))),
        }
    }
}

bindings::export!(Plugin with_types_in bindings);
