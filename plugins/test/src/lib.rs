use api::command::{self, Arity, Command, CommandError, CommandFlags};
use api::dependency;
use api::lifecycle::{self, InitializeResult};
use bindings::zi::api::editor::*;

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

impl bindings::exports::zi::api::command::Guest for Component {
    type Handler = CommandHandler;
}

struct CommandHandler;

impl command::GuestHandler for CommandHandler {
    fn new() -> Self {
        Self
    }

    fn exec(&self, cmd: String, _args: Vec<String>, _force: bool) -> Result<(), CommandError> {
        match cmd.as_str() {
            "foo" => Ok(()),
            "stale-view" => {
                use_closed_view();
                Ok(())
            }
            _ => Err(CommandError::Message(format!("unexpected command: `{cmd}`"))),
        }
    }
}

impl lifecycle::Guest for Component {
    fn initialize() -> InitializeResult {
        test_cursor();
        test_insert_delete_undo_redo();
        test_buffer_apis();
        test_view_scroll_align_focus();
        test_mode();

        InitializeResult {
            commands: vec![
                Command {
                    name: "foo".into(),
                    arity: Arity { min: 0, max: 1 },
                    opts: CommandFlags::RANGE,
                },
                Command {
                    name: "stale-view".into(),
                    arity: Arity { min: 0, max: 0 },
                    opts: CommandFlags::empty(),
                },
            ],
        }
    }

    fn shutdown() {}
}

fn test_cursor() {
    let view = get_active_view();
    assert_eq!(view.get_cursor(), Point { line: 0, byte_col: 0 });
    view.set_cursor(Point { line: 0, byte_col: 1 });
    assert_eq!(view.get_cursor(), Point { line: 0, byte_col: 0 });
    insert("abc").unwrap();
    view.set_cursor(Point { line: 0, byte_col: 1 });
    assert_eq!(view.get_cursor(), Point { line: 0, byte_col: 1 });
}

fn test_insert_delete_undo_redo() {
    let view = get_active_view();
    view.set_cursor(Point { line: 0, byte_col: 0 });

    view.insert("hello").unwrap();

    let buf = view.get_buffer();
    assert!(buf.len_lines() >= 1);

    view.set_cursor(Point { line: 0, byte_col: 4 });
    view.delete_char().unwrap();

    let undone = view.undo().unwrap();
    assert!(undone);

    let redone = view.redo().unwrap();
    assert!(redone);
}

fn test_buffer_apis() {
    let view = get_active_view();
    let buf = view.get_buffer();

    let _path = buf.get_file_path();

    let flags = buf.get_flags();
    assert!(flags.contains(BufferFlags::DIRTY));

    let lines = buf.len_lines();
    assert!(lines >= 1);
}

fn use_closed_view() {
    let view = get_active_view();
    view.inspect();
    let buf = view.get_buffer();
    assert!(view.exists());
    assert!(buf.exists());

    view.close();
    assert!(!view.exists());
    assert!(!buf.exists());

    view.get_cursor();
}

fn test_view_scroll_align_focus() {
    let view = get_active_view();
    view.scroll(Direction::Down, 1);
    view.scroll(Direction::Up, 1);
    view.align(VerticalAlignment::Center);
    view.align(VerticalAlignment::Top);
    view.align(VerticalAlignment::Bottom);
    view.focus();
}

fn test_mode() {
    assert_eq!(get_mode(), Mode::Normal);
    set_mode(Mode::Insert);
    assert_eq!(get_mode(), Mode::Insert);
    set_mode(Mode::Normal);
    assert_eq!(get_mode(), Mode::Normal);
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
