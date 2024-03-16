use bindings::{Guest, Name};

use self::bindings::exports::zi::api::command::{self, Command};

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

    fn commands() -> Vec<Command> {
        vec![]
    }
}

struct CommandHandler;

impl command::GuestHandler for CommandHandler {
    fn new() -> Self {
        Self
    }

    fn exec(&self, _cmd: String, _args: Vec<String>) -> u32 {
        42
    }
}

impl Guest for Component {
    fn initialize() {}

    fn get_name() -> Name {
        "example".into()
    }

    fn dependencies() -> Vec<Name> {
        vec![]
    }
}

bindings::export!(Component with_types_in bindings);
