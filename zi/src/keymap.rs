use crossterm::event::KeyCode;
use rustc_hash::FxHashMap;

use crate::event::KeyEvent;
use crate::{motion, Direction, Editor, Mode};

#[derive(Default)]
pub(crate) struct Keymap {
    maps: FxHashMap<Mode, Trie>,
    last_mode: Option<Mode>,
    // commands to execute
    actions: Vec<Action>,
}

pub enum Action {
    Fn(fn(&mut Editor)),
    Insert(char),
}

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
        use Action::*;
        match key.code {
            KeyCode::Esc if matches!(mode, Mode::Insert) => {
                return Some(Fn(|editor| editor.set_mode(Mode::Normal)));
            }
            KeyCode::Enter if matches!(mode, Mode::Insert) => return Some(Insert('\n')),
            KeyCode::Left => return Some(Fn(|editor| editor.move_active_cursor(Direction::Left))),
            KeyCode::Right => {
                return Some(Fn(|editor| editor.move_active_cursor(Direction::Right)));
            }
            KeyCode::Up => return Some(Fn(|editor| editor.move_active_cursor(Direction::Up))),
            KeyCode::Down => return Some(Fn(|editor| editor.move_active_cursor(Direction::Down))),
            KeyCode::Char(c) => match c {
                'I' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| {
                        editor.set_active_cursor(editor.active_cursor().with_col(0));
                        editor.set_mode(Mode::Insert);
                    }));
                }
                'W' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| editor.motion(motion::NextToken)));
                }
                'w' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| editor.motion(motion::NextWord)));
                }
                'o' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| {
                        editor.set_mode(Mode::Insert);
                        editor.set_active_cursor(editor.active_cursor().with_col(u32::MAX));
                        editor.insert_char('\n');
                    }));
                }
                'A' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| {
                        editor.set_active_cursor(editor.active_cursor().with_col(u32::MAX));
                        editor.set_mode(Mode::Insert);
                        editor.move_active_cursor(Direction::Right);
                    }));
                }
                'i' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| editor.set_mode(Mode::Insert)));
                }
                'a' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| {
                        editor.set_mode(Mode::Insert);
                        editor.move_active_cursor(Direction::Right);
                    }));
                }
                'q' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| editor.quit = true));
                }
                'h' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| editor.move_active_cursor(Direction::Left)));
                }
                'l' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| editor.move_active_cursor(Direction::Right)));
                }
                'j' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| editor.move_active_cursor(Direction::Down)));
                }
                'k' if matches!(mode, Mode::Normal) => {
                    return Some(Fn(|editor| editor.move_active_cursor(Direction::Up)));
                }
                'f' if matches!(mode, Mode::Insert) => {
                    return Some(Fn(|editor| editor.set_mode(Mode::Normal)));
                }
                c if matches!(mode, Mode::Insert) => {
                    return Some(Insert(c));
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
