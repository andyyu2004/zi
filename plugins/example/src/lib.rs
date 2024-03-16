use api::lifecycle::{self, InitializeResult};
use api::{command, dependency};

use self::bindings::exports::zi::api;
use self::bindings::exports::zi::api::command::Command;
use self::bindings::zi::api::editor::{self, Mode};

struct Component;

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../zi/wit/zi.wit",
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

    fn exec(&self, cmd: String, _args: Vec<String>) {
        match &cmd[..] {
            "foo" => editor::set_mode(Mode::Insert),
            _ => unreachable!("unexpected command: `{cmd}`"),
        }
    }
}

impl lifecycle::Guest for Component {
    fn initialize() -> InitializeResult {
        InitializeResult {
            commands: vec![Command {
                name: "foo".into(),
                arity: api::command::Arity { min: 0, max: 1 },
                opts: api::command::CommandFlags::empty(),
            }],
        }
    }
    fn shutdown() {}
}

impl dependency::Guest for Component {
    fn get_name() -> String {
        "example".into()
    }

    fn dependencies() -> Vec<String> {
        vec![]
    }
}

bindings::export!(Component with_types_in bindings);
