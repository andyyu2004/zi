mod bindings;

use bindings::Guest;

use crate::bindings::zi::api::editor::{get_mode, set_mode, Mode};

struct Component;

impl Guest for Component {
    fn initialize() {
        assert_eq!(get_mode(), Mode::Normal);
        set_mode(Mode::Insert);
        assert_eq!(get_mode(), Mode::Insert);
    }
}
