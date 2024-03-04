use std::hash::Hash;
use std::iter;

use rustc_hash::FxHashMap;
use stdx::merge::Merge;

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
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(maps: FxHashMap<M, Trie<K, V>>) -> Self {
        Self { maps, buffer: Default::default(), last_mode: Default::default() }
    }

    // This method should be useful eventually, just cfg it to hide warnings
    #[cfg(test)]
    pub fn insert(&mut self, mode: M, keys: impl IntoIterator<Item = K>, value: V) -> Option<V> {
        self.maps.entry(mode).or_default().insert(keys.into_iter().peekable(), value)
    }

    /// Returns the result of the key sequence and the keys that were discarded
    pub fn on_key(&mut self, mode: M, key: K) -> (TrieResult<V>, Vec<K>) {
        if let Some(last_mode) = &self.last_mode {
            if last_mode != &mode {
                self.buffer.clear();
                self.last_mode = Some(mode.clone());
            }
        } else {
            self.last_mode = Some(mode.clone());
        }

        let trie = match self.maps.get(&mode) {
            Some(trie) => trie,
            None => return (TrieResult::Nothing, vec![key]),
        };

        self.buffer.push(key);

        let v = trie.get(self.buffer.iter());
        match v {
            TrieResult::Found(v) => {
                self.buffer.clear();
                (TrieResult::Found(v.clone()), vec![])
            }
            TrieResult::Partial => (TrieResult::Partial, vec![]),
            TrieResult::Nothing => {
                let mut cancelled = std::mem::take(&mut self.buffer);
                let key = cancelled.last().expect("buffer can't be empty");

                // Start a new sequence with the key that wasn't found
                let trie = self.maps.get(&mode).expect("we wouldn't be here if this didn't exist");
                // We check if the key could potentially be the start of a new sequence
                if let TrieResult::Nothing = trie.get(iter::once(key)) {
                    // If not, we don't recurse. (necessarily to avoid infinite loop)
                    return (TrieResult::Nothing, cancelled);
                }

                // We now pop off the key from the discard as it's being used
                let key = cancelled.pop().expect("we just checked `last` exists");
                match self.on_key(mode, key).0 {
                    TrieResult::Found(v) => (TrieResult::Found(v), cancelled),
                    TrieResult::Partial => (TrieResult::Partial, cancelled),
                    TrieResult::Nothing => unreachable!("just checked it above"),
                }
            }
        }
    }
}

impl<M, K, V> Default for Keymap<M, K, V> {
    fn default() -> Self {
        Self { maps: Default::default(), buffer: Default::default(), last_mode: Default::default() }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Trie<K, V> {
    children: FxHashMap<K, TrieNode<K, V>>,
}

impl<K: Eq + Hash, V> Merge for Trie<K, V> {
    fn merge(self, other: Self) -> Self {
        let children = self.children.merge(other.children);
        Self { children }
    }
}

impl<K, V> Trie<K, V> {
    pub(crate) fn new(children: FxHashMap<K, TrieNode<K, V>>) -> Self {
        Self { children }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TrieResult<V> {
    Found(V),
    Partial,
    Nothing,
}

impl<K, V> Trie<K, V>
where
    K: Eq + Hash,
{
    fn get<'a>(&self, mut keys: impl Iterator<Item = &'a K>) -> TrieResult<&V>
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
            None => TrieResult::Nothing,
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

#[derive(Debug, Clone)]
pub(crate) enum TrieNode<K, V> {
    Trie(Trie<K, V>),
    Value(V),
}

impl<K: Eq + Hash, V> Merge for TrieNode<K, V> {
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Trie(a), Self::Trie(b)) => Self::Trie(a.merge(b)),
            (_, b) => b,
        }
    }
}

impl<K, V> TrieNode<K, V> {
    pub(crate) fn into_trie(self) -> Trie<K, V> {
        if let Self::Trie(v) = self { v } else { panic!("Expected Trie") }
    }
}

#[cfg(test)]
mod tests;
