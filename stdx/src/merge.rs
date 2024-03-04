use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};

/// A right-biased merge operation.
pub trait Merge {
    fn merge(self, other: Self) -> Self;
}

impl<K, V, S> Merge for HashMap<K, V, S>
where
    K: Eq + Hash,
    V: Merge,
    S: BuildHasher,
{
    fn merge(mut self, other: Self) -> Self {
        for (k, v) in other {
            match self.remove(&k) {
                Some(x) => self.insert(k, x.merge(v)),
                None => self.insert(k, v),
            };
        }
        self
    }
}
