use std::fmt;

use smallvec::SmallVec;
use tinyset::SetU64;

use crate::DebugSet;
use crate::key::{Flags, Key};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Extent {
    pub(crate) length: usize,
    /// The ids contained within this range.
    /// All their positions is considered to be the start of the extent.
    pub(crate) keys: SetU64,
}

impl fmt::Debug for Extent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("").field(&self.length).field(&DebugSet(|| self.keys())).finish()
    }
}

impl Extent {
    #[track_caller]
    pub(crate) fn new(length: usize, keys: impl IntoIterator<Item = Key>) -> Self {
        Self { length, keys: keys.into_iter().map(Key::into_raw).collect() }
    }

    pub(crate) fn ids(&self) -> impl Iterator<Item = u64> + '_ {
        self.keys().map(Key::id)
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = Key> + '_ {
        self.keys.iter().map(Key::from_raw)
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.length
    }

    pub(crate) fn remove(&mut self, id: u64) -> bool {
        if self.keys.remove(Key::new(id, Flags::empty()).into_raw()) {
            return true;
        }
        let key = self.keys.iter().find(|&key| Key::from_raw(key).id() == id);
        key.is_some_and(|key| self.keys.remove(key))
    }
}

#[derive(Debug)]
pub(super) struct ExtentBuilder<const N: usize> {
    extents: SmallVec<Extent, N>,
}

impl<const N: usize> Default for ExtentBuilder<N> {
    fn default() -> Self {
        Self { extents: SmallVec::new() }
    }
}

impl<const N: usize> ExtentBuilder<N> {
    pub fn finish(self) -> SmallVec<Extent, N> {
        self.extents
    }

    #[track_caller]
    pub fn push(&mut self, length: usize, keys: impl IntoIterator<Item = Key>) {
        let mut keys = keys.into_iter().peekable();
        match self.extents.last_mut() {
            Some(last) if last.length == 0 || keys.peek().is_none() => {
                // Merge entries if possible.
                last.length += length;
                last.keys.extend(keys.map(Key::into_raw));
            }
            _ => self.extents.push(Extent::new(length, keys)),
        }
    }

    #[track_caller]
    pub fn push_raw(&mut self, length: usize, keys: impl IntoIterator<Item = u64>) {
        self.push(length, keys.into_iter().map(Key::from_raw));
    }

    pub fn push_gap(&mut self, gap: usize) {
        if gap > 0 {
            self.push(gap, []);
        }
    }

    #[track_caller]
    pub fn push_extent(&mut self, extent: &Extent) {
        self.push_raw(extent.len(), extent.keys().map(Key::into_raw));
    }
}
