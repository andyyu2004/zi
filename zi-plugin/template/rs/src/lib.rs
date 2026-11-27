use bindings::exports::zi::api::command::{self, Arity, Command, CommandError, CommandFlags};
use bindings::exports::zi::api::{dependency, lifecycle};
use bindings::zi::api::editor::{self, Mode};

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({ path: "wit", world: "plugin" });
}

struct Plugin;

impl dependency::Guest for Plugin {
    fn get_name() -> String {
        "zi-plugin-template".into()
    }

    fn dependencies() -> Vec<String> {
        vec![]
    }
}

impl lifecycle::Guest for Plugin {
    fn initialize() -> lifecycle::InitializeResult {
        lifecycle::InitializeResult {
            commands: vec![Command {
                name: "hello".into(),
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
            "hello" => {
                editor::set_mode(Mode::Insert);
                Ok(())
            }
            _ => Err(CommandError::Message(format!("unknown command: `{cmd}`"))),
        }
    }
}

bindings::export!(Plugin with_types_in bindings);
