use rustc_hash::FxHashMap;

use crate::event::KeyEvent;
use crate::{Editor, Mode};

#[derive(Default)]
pub(crate) struct Keymap {
    maps: FxHashMap<Mode, Trie>,
    last_mode: Option<Mode>,
    // commands to execute
    actions: Vec<Action>,
}

type Action = fn(&mut Editor);

// macro_rules! keymap {
//     ( $($mode:ident {
//        $( $key:expr => $value:expr, )*
//     } )+ ) => {
//     };
// }

// keymap! {
//     Normal {
//         "i" => |store| {},
//     }
//     Insert {
//         "f" => {
//             "d" => Mode::Normal,
//         }
//     }
// }

impl Keymap {
    pub fn on_key(&mut self, mode: Mode, key: KeyEvent) -> Option<Action> {
        // temp
        if let crossterm::event::KeyCode::Char(c) = key.code {
            match c {
                'i' if matches!(mode, Mode::Insert) => {
                    return Some(|editor| editor.set_mode(Mode::Insert));
                }
                'q' if matches!(mode, Mode::Normal) => {
                    return Some(|editor| editor.quit = true);
                }
                'f' if matches!(mode, Mode::Insert) => {
                    return Some(|editor| editor.set_mode(Mode::Normal));
                }
                _ => (),
            }
        }
        None
        // if let Some(last_mode) = self.last_mode {
        //     if last_mode != mode {
        //         self.buffer.clear();
        //     }
        // }
        //
        // self.last_mode = Some(mode);
        // todo!()
    }
}

struct Trie {
    children: FxHashMap<KeyEvent, Trie>,
}

enum KeyTrieNode {
    Trie(Trie),
    Action(Action),
}

#[cfg(test)]
mod tests;
