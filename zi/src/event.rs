pub use crossterm::event::{KeyCode, KeyModifiers};

pub enum Event {
    Key(KeyEvent),
}

pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}
