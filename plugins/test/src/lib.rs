use api::command::{self, Arity, Command, CommandFlags};
use api::dependency;
use api::lifecycle::{self, InitializeResult};
use bindings::zi::api::editor::*;

use self::bindings::exports::zi::api;

struct Component;

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../zi/wit/zi.wit",
        additional_derives: [PartialEq, Eq],
        ownership: Borrowing { duplicate_if_necessary: true },
    });
}

impl bindings::exports::zi::api::command::Guest for Component {
    type Handler = CommandHandler;
}

struct CommandHandler;

impl command::GuestHandler for CommandHandler {
    fn new() -> Self {
        Self
    }

    fn exec(&self, cmd: String, _args: Vec<String>) {
        assert_eq!(cmd, "foo", "unexpected command");
    }
}

impl lifecycle::Guest for Component {
    fn initialize() -> InitializeResult {
        let view = get_active_view();
        assert_eq!(view.get_cursor(), Position { line: 0, col: 0 });
        view.set_cursor(Position { line: 0, col: 1 });
        assert_eq!(view.get_cursor(), Position { line: 0, col: 0 });
        insert("abc");
        view.set_cursor(Position { line: 0, col: 1 });
        assert_eq!(view.get_cursor(), Position { line: 0, col: 1 });

        let _buf = view.get_buffer();

        InitializeResult {
            commands: vec![Command {
                name: "foo".into(),
                arity: Arity { min: 0, max: 1 },
                opts: CommandFlags::RANGE,
            }],
        }
    }

    fn shutdown() {}
}

impl dependency::Guest for Component {
    fn get_name() -> String {
        "test".into()
    }

    fn dependencies() -> Vec<String> {
        vec![]
    }
}

bindings::export!(Component with_types_in bindings);
