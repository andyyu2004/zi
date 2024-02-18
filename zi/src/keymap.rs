use crossterm::event::KeyCode;
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
        match key.code {
            KeyCode::Esc if matches!(mode, Mode::Insert) => {
                return Some(|editor| editor.set_mode(Mode::Normal));
            }
            KeyCode::Char(c) => match c {
                'i' if matches!(mode, Mode::Normal) => {
                    return Some(|editor| editor.set_mode(Mode::Insert));
                }
                'q' if matches!(mode, Mode::Normal) => {
                    return Some(|editor| editor.quit = true);
                }
                'h' if matches!(mode, Mode::Normal) => {
                    return Some(|editor| {
                        let view = editor.active_view();
                        editor.set_cursor(view.id(), view.cursor().left(1))
                    });
                }
                'l' if matches!(mode, Mode::Normal) => {
                    return Some(|editor| {
                        let view = editor.active_view();
                        editor.set_cursor(view.id(), view.cursor().right(1))
                    });
                }
                'j' if matches!(mode, Mode::Normal) => {
                    return Some(|editor| {
                        let view = editor.active_view();
                        editor.set_cursor(view.id(), view.cursor().down(1))
                    });
                }
                'k' if matches!(mode, Mode::Normal) => {
                    return Some(|editor| {
                        let view = editor.active_view();
                        editor.set_cursor(view.id(), view.cursor().up(1))
                    });
                }
                'f' if matches!(mode, Mode::Insert) => {
                    return Some(|editor| editor.set_mode(Mode::Normal));
                }
                char if matches!(mode, Mode::Insert) => {
                    return Some(|editor| {
                        let (view, buf) = editor.active();
                        let cursor = view.cursor();
                    });
                }
                _ => (),
            },
            _ => (),
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
