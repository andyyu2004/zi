use std::hash::Hash;

use rustc_hash::FxHashMap;

mod macros;

#[derive(Debug)]
pub struct Keymap<M, K, V> {
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

    // This method should be useful eventually, just cfg it to hide warnings
    #[cfg(test)]
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
                let push = !self.buffer.is_empty();
                self.buffer.clear();
                if push {
                    // The non-existent key should become the start of a new sequence
                    // if wasn't the only key in the buffer.
                    // This is sort of a heuristic to make it dwim.
                    self.buffer.push(key);
                }
                None
            }
        }
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

    #[cfg(test)]
    fn insert<I: Iterator<Item = K>>(
        &mut self,
        mut keys: std::iter::Peekable<I>,
        value: V,
    ) -> Option<V> {
        use std::collections::hash_map::Entry;
        let k = keys.next().expect("cannot insert empty sequence of keys");

        let mk_new_node = |mut keys: std::iter::Peekable<I>, value: V| {
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
