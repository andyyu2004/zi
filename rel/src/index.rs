use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::Hash;
use std::ops::{Bound, RangeBounds};

type Ids<'a, I> = Box<dyn Iterator<Item = I> + 'a>;

/// Index lookups usable without knowing the index kind, for [`crate::Column`].
pub trait LookupIndex<K, I> {
    fn lookup(&self, key: &K) -> Ids<'_, I>;

    /// `None` if the index is not ordered.
    fn range(&self, range: (Bound<K>, Bound<K>)) -> Option<Ids<'_, I>>;
}

/// A hash index from a key to every row with that key.
pub struct HashIndex<K, I> {
    map: HashMap<K, BTreeSet<I>>,
}

/// A hash index from a key to the one row with that key.
pub struct UniqueHashIndex<K, I> {
    map: HashMap<K, I>,
}

/// An ordered index from a key to every row with that key.
pub struct OrdIndex<K, I> {
    map: BTreeMap<K, BTreeSet<I>>,
}

/// An ordered index from a key to the one row with that key.
pub struct UniqueOrdIndex<K, I> {
    map: BTreeMap<K, I>,
}

macro_rules! default_impl {
    ($($ty:ident),*) => {$(
        impl<K, I> Default for $ty<K, I> {
            fn default() -> Self {
                Self { map: Default::default() }
            }
        }
    )*};
}

default_impl!(HashIndex, UniqueHashIndex, OrdIndex, UniqueOrdIndex);

impl<K: Hash + Eq, I: Ord + Copy> HashIndex<K, I> {
    pub fn get<Q>(&self, key: &Q) -> impl Iterator<Item = I> + '_ + use<'_, K, I, Q>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.get(key).into_iter().flatten().copied()
    }

    pub fn insert(&mut self, key: K, id: I) {
        self.map.entry(key).or_default().insert(id);
    }

    pub fn remove(&mut self, key: &K, id: I) {
        if let Some(ids) = self.map.get_mut(key) {
            ids.remove(&id);
            if ids.is_empty() {
                self.map.remove(key);
            }
        }
    }
}

impl<K: Hash + Eq, I: Ord + Copy> UniqueHashIndex<K, I> {
    pub fn get<Q>(&self, key: &Q) -> Option<I>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.map.get(key).copied()
    }

    pub fn contains(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    pub fn insert(&mut self, key: K, id: I) {
        self.map.insert(key, id);
    }

    pub fn remove(&mut self, key: &K, _id: I) {
        self.map.remove(key);
    }
}

impl<K: Ord, I: Ord + Copy> OrdIndex<K, I> {
    pub fn get<Q>(&self, key: &Q) -> impl Iterator<Item = I> + '_ + use<'_, K, I, Q>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.map.get(key).into_iter().flatten().copied()
    }

    pub fn range<R: RangeBounds<K>>(&self, range: R) -> impl Iterator<Item = I> + '_ {
        self.map.range(range).flat_map(|(_, ids)| ids.iter().copied())
    }

    pub fn insert(&mut self, key: K, id: I) {
        self.map.entry(key).or_default().insert(id);
    }

    pub fn remove(&mut self, key: &K, id: I) {
        if let Some(ids) = self.map.get_mut(key) {
            ids.remove(&id);
            if ids.is_empty() {
                self.map.remove(key);
            }
        }
    }
}

impl<K: Ord, I: Ord + Copy> UniqueOrdIndex<K, I> {
    pub fn get<Q>(&self, key: &Q) -> Option<I>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.map.get(key).copied()
    }

    pub fn range<R: RangeBounds<K>>(&self, range: R) -> impl Iterator<Item = I> + '_ {
        self.map.range(range).map(|(_, id)| *id)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    pub fn insert(&mut self, key: K, id: I) {
        self.map.insert(key, id);
    }

    pub fn remove(&mut self, key: &K, _id: I) {
        self.map.remove(key);
    }
}

impl<K: Hash + Eq, I: Ord + Copy> LookupIndex<K, I> for HashIndex<K, I> {
    fn lookup(&self, key: &K) -> Ids<'_, I> {
        Box::new(self.get(key))
    }

    fn range(&self, _: (Bound<K>, Bound<K>)) -> Option<Ids<'_, I>> {
        None
    }
}

impl<K: Hash + Eq, I: Ord + Copy> LookupIndex<K, I> for UniqueHashIndex<K, I> {
    fn lookup(&self, key: &K) -> Ids<'_, I> {
        Box::new(self.get(key).into_iter())
    }

    fn range(&self, _: (Bound<K>, Bound<K>)) -> Option<Ids<'_, I>> {
        None
    }
}

impl<K: Ord, I: Ord + Copy> LookupIndex<K, I> for OrdIndex<K, I> {
    fn lookup(&self, key: &K) -> Ids<'_, I> {
        Box::new(self.get(key))
    }

    fn range(&self, range: (Bound<K>, Bound<K>)) -> Option<Ids<'_, I>> {
        Some(Box::new(OrdIndex::range(self, range)))
    }
}

impl<K: Ord, I: Ord + Copy> LookupIndex<K, I> for UniqueOrdIndex<K, I> {
    fn lookup(&self, key: &K) -> Ids<'_, I> {
        Box::new(self.get(key).into_iter())
    }

    fn range(&self, range: (Bound<K>, Bound<K>)) -> Option<Ids<'_, I>> {
        Some(Box::new(UniqueOrdIndex::range(self, range)))
    }
}
