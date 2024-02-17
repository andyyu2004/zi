use crate::event::KeyEvent;
use crate::keymap::Keymap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
}

#[derive(Default)]
pub struct Editor {
    pub(crate) mode: Mode,
    keymap: Keymap,
    pub quit: bool, // tmp hack
}

impl Editor {
    pub fn on_key(&mut self, key: KeyEvent) {
        if let Some(f) = self.keymap.on_key(self.mode, key) {
            f(self)
        }
    }
}
