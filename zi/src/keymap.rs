use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::iter::Peekable;

use rustc_hash::FxHashMap;

mod macros;

#[derive(Debug)]
pub(crate) struct Keymap<M, K, V> {
    maps: FxHashMap<M, Trie<K, V>>,
    /// The keys that have been pressed so far
    buffer: Vec<K>,
    /// The last mode that was used
    last_mode: Option<M>,
}

impl<M, K, V> Keymap<M, K, V>
where
    M: Eq + Hash + Clone,
    K: Eq + Hash,
{
    pub fn new(maps: FxHashMap<M, Trie<K, V>>) -> Self {
        Self { maps, buffer: Default::default(), last_mode: Default::default() }
    }

    pub fn insert(&mut self, mode: M, keys: impl IntoIterator<Item = K>, value: V) -> Option<V> {
        self.maps.entry(mode).or_default().insert(keys.into_iter().peekable(), value)
    }

    pub fn on_key(&mut self, mode: M, key: K) -> Option<&V> {
        if let Some(last_mode) = &self.last_mode {
            if last_mode != &mode {
                self.buffer.clear();
                self.last_mode = Some(mode.clone());
            }
        } else {
            self.last_mode = Some(mode.clone());
        }

        self.buffer.push(key);

        let v = self.maps.get(&mode)?.get(self.buffer.iter());
        match v {
            TrieResult::Found(v) => {
                self.buffer.clear();
                Some(v)
            }
            TrieResult::Partial => None,
            TrieResult::NotFound => {
                let key = self.buffer.pop().expect("we just pushed `key`");
                self.buffer.clear();
                // The non-existent key should become the start of a new sequence
                self.buffer.push(key);
                None
            }
        }

        // todo!()
        // temp
        // match key.code {
        //     KeyCode::Left => return Some(Fn(|editor| editor.move_active_cursor(Direction::Left))),
        //     KeyCode::Right => {
        //         return Some(Fn(|editor| editor.move_active_cursor(Direction::Right)));
        //     }
        //     KeyCode::Up => return Some(Fn(|editor| editor.move_active_cursor(Direction::Up))),
        //     KeyCode::Down => return Some(Fn(|editor| editor.move_active_cursor(Direction::Down))),
        //     KeyCode::Char(c) => match c {
        //         'I' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| {
        //                 editor.set_active_cursor(editor.active_cursor().with_col(0));
        //                 editor.set_mode(Mode::Insert);
        //             }));
        //         }
        //         'W' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| editor.motion(motion::NextToken)));
        //         }
        //         'w' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| editor.motion(motion::NextWord)));
        //         }
        //         'b' if matches!(mode, Mode::Normal) => {
        //             // todo prev word
        //             return Some(Fn(|editor| editor.motion(motion::PrevToken)));
        //         }
        //         'B' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| editor.motion(motion::PrevToken)));
        //         }
        //         'g' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| editor.go_to_definition()));
        //         }
        //         'o' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| {
        //                 editor.set_mode(Mode::Insert);
        //                 editor.set_active_cursor(editor.active_cursor().with_col(u32::MAX));
        //                 editor.insert_char('\n');
        //             }));
        //         }
        //         'A' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| {
        //                 editor.set_active_cursor(editor.active_cursor().with_col(u32::MAX));
        //                 editor.set_mode(Mode::Insert);
        //                 editor.move_active_cursor(Direction::Right);
        //             }));
        //         }
        //         'i' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| editor.set_mode(Mode::Insert)));
        //         }
        //         'a' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| {
        //                 editor.set_mode(Mode::Insert);
        //                 editor.move_active_cursor(Direction::Right);
        //             }));
        //         }
        //         'q' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| editor.close_active_view()));
        //         }
        //         'h' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| editor.move_active_cursor(Direction::Left)));
        //         }
        //         'l' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| editor.move_active_cursor(Direction::Right)));
        //         }
        //         'j' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| editor.move_active_cursor(Direction::Down)));
        //         }
        //         'k' if matches!(mode, Mode::Normal) => {
        //             return Some(Fn(|editor| editor.move_active_cursor(Direction::Up)));
        //         }
        //         'f' if matches!(mode, Mode::Insert) => {
        //             return Some(Fn(|editor| editor.set_mode(Mode::Normal)));
        //         }
        //         c if matches!(mode, Mode::Insert) => {
        //             return Some(Insert(c));
        //         }
        //         _ => (),
        //     },
        //     _ => (),
        // }
    }
}

impl<M, K, V> Default for Keymap<M, K, V> {
    fn default() -> Self {
        Self { maps: Default::default(), buffer: Default::default(), last_mode: Default::default() }
    }
}

#[derive(Debug)]
pub(crate) struct Trie<K, V> {
    children: FxHashMap<K, TrieNode<K, V>>,
}

impl<K, V> Trie<K, V> {
    pub(crate) fn new(children: FxHashMap<K, TrieNode<K, V>>) -> Self {
        Self { children }
    }
}

enum TrieResult<'a, V> {
    Found(&'a V),
    Partial,
    NotFound,
}

impl<K, V> Trie<K, V>
where
    K: Eq + Hash,
{
    fn get<'a>(&self, mut keys: impl Iterator<Item = &'a K>) -> TrieResult<'_, V>
    where
        K: 'a,
    {
        let k = match keys.next() {
            Some(k) => k,
            None => return TrieResult::Partial,
        };

        match self.children.get(k) {
            Some(TrieNode::Trie(trie)) => trie.get(keys),
            Some(TrieNode::Value(v)) => TrieResult::Found(v),
            None => TrieResult::NotFound,
        }
    }

    fn insert<I: Iterator<Item = K>>(&mut self, mut keys: Peekable<I>, value: V) -> Option<V> {
        let k = keys.next().expect("cannot insert empty sequence of keys");

        let mk_new_node = |mut keys: Peekable<I>, value: V| {
            if keys.peek().is_some() {
                let mut trie = Trie::default();
                trie.insert(keys, value);
                TrieNode::Trie(trie)
            } else {
                TrieNode::Value(value)
            }
        };

        match self.children.entry(k) {
            Entry::Occupied(mut entry) => match entry.get_mut() {
                TrieNode::Trie(trie) => trie.insert(keys, value),
                TrieNode::Value(_) => match entry.insert(mk_new_node(keys, value)) {
                    TrieNode::Value(v) => Some(v),
                    _ => unreachable!("we know it's a value"),
                },
            },
            Entry::Vacant(entry) => {
                entry.insert(mk_new_node(keys, value));
                None
            }
        }
    }
}

impl<K, V> Default for Trie<K, V> {
    fn default() -> Self {
        Self { children: Default::default() }
    }
}

#[derive(Debug)]
pub(crate) enum TrieNode<K, V> {
    Trie(Trie<K, V>),
    Value(V),
}

impl<K, V> TrieNode<K, V> {
    pub(crate) fn into_trie(self) -> Trie<K, V> {
        if let Self::Trie(v) = self { v } else { panic!("Expected Trie") }
    }
}

#[cfg(test)]
mod tests;
