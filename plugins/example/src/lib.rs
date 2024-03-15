// #[allow(warnings)]
// mod bindings;

use bindings::zi::api::editor::*;
use bindings::{Guest, Name};

use self::bindings::exports::zi::api::plugin_resources::GuestCommandHandler;
use self::bindings::CommandHandler;

struct Component;

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../zi/wit/zi.wit",
        additional_derives: [PartialEq, Eq],
        ownership: Borrowing { duplicate_if_necessary: true },
    });
}

impl bindings::exports::zi::api::plugin_resources::Guest for Component {
    type CommandHandler = HandlerImpl;
}

struct HandlerImpl;

impl GuestCommandHandler for HandlerImpl {
    fn new() -> Self {
        Self
    }

    fn exec(&self, cmd: String, args: Vec<String>) {}
}

impl Guest for Component {
    fn initialize() {
        assert_eq!(get_mode(), Mode::Normal);
        set_mode(Mode::Insert);
        assert_eq!(get_mode(), Mode::Insert);

        let view = get_active_view();
        assert_eq!(view.get_cursor(), Position { line: 0, col: 0 });
        view.set_cursor(Position { line: 0, col: 1 });
        assert_eq!(view.get_cursor(), Position { line: 0, col: 0 });
        insert("abc");
        view.set_cursor(Position { line: 0, col: 1 });
        assert_eq!(view.get_cursor(), Position { line: 0, col: 1 });

        let _buf = view.get_buffer();
        // assert!();
    }

    fn get_name() -> Name {
        "example".into()
    }

    fn dependencies() -> Vec<Name> {
        vec![]
    }

    fn handler() -> CommandHandler {
        CommandHandler::new()
    }
}

bindings::export!(Component with_types_in bindings);
