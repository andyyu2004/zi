mod bindings;

use bindings::Guest;

use crate::bindings::zi::api::editor::*;

struct Component;

impl Guest for Component {
    fn initialize() {
        assert_eq!(get_mode(), Mode::Normal);
        set_mode(Mode::Insert);
        assert_eq!(get_mode(), Mode::Insert);

        let view = get_active_view();
        assert_eq!(view.get_cursor(), Position { line: 0, col: 0 });
        let buf = view.get_buffer();
        // assert!();
    }
}
