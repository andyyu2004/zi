use bindings::zi::api::editor::*;
use bindings::{Guest, Name};

use self::bindings::exports::zi::api::command;

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

    fn exec(&self, _cmd: String, _args: Vec<String>) -> u32 {
        42
    }
}

impl Guest for Component {
    fn initialize() {
        let view = get_active_view();
        assert_eq!(view.get_cursor(), Position { line: 0, col: 0 });
        view.set_cursor(Position { line: 0, col: 1 });
        assert_eq!(view.get_cursor(), Position { line: 0, col: 0 });
        insert("abc");
        view.set_cursor(Position { line: 0, col: 1 });
        assert_eq!(view.get_cursor(), Position { line: 0, col: 1 });

        let _buf = view.get_buffer();
    }

    fn get_name() -> Name {
        "test".into()
    }

    fn dependencies() -> Vec<Name> {
        vec![]
    }
}

bindings::export!(Component with_types_in bindings);
