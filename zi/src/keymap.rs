use rustc_hash::FxHashMap;

use crate::event::KeyEvent;
use crate::Mode;

pub(crate) struct Keymap {
    maps: FxHashMap<Mode, Trie>,
    last_mode: Option<Mode>,
    // commands to execute
    buffer: Vec<()>,
}

macro_rules! keymap {
    ( $($mode:ident {
       $( $key:expr => $value:expr, )*
    } )+ ) => {};
}

keymap! {
    Normal {
        "i" => Mode::Insert,
    }
    Insert {
        // "f" => {
        //     "d" => Mode::Normal,
        // }
    }
}

impl Keymap {
    pub fn new() -> Keymap {
        todo!()
    }

    pub fn on_key(&mut self, mode: Mode, key: KeyEvent) {
        if let Some(last_mode) = self.last_mode {
            if last_mode != mode {
                self.buffer.clear();
            }
        }

        self.last_mode = Some(mode);
        todo!()
    }
}

struct Trie {
    children: FxHashMap<KeyEvent, Trie>,
}

enum KeyTrieNode {
    Trie(Trie),
    Command,
}

#[cfg(test)]
mod tests;
