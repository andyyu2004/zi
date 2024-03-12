#[allow(warnings)]
mod bindings;

use bindings::zi::api::editor::*;
use bindings::Guest;

use self::bindings::Dependency;

struct Component;

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

    fn dependencies() -> Vec<Dependency> {
        vec![]
    }
}

bindings::export!(Component with_types_in bindings);
