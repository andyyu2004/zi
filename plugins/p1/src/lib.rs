#[allow(clippy::all, dead_code)]
mod bindings;
use bindings::Guest;

struct Component;

impl Guest for Component {
    fn greet(_input: String) -> String {
        "Hello, World!".to_string()
    }
}
