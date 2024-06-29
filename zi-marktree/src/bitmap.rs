use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::{BitOrAssign, SubAssign};

use roaring::RoaringBitmap;

/// A bitmap with 48-bit values that stores duplicity up to `N`.
// The `i`th bitmap stores the values that are present at least `i` times.
#[derive(Clone, PartialEq)]
pub(crate) struct Bitbag48<const N: usize>([Bitmap48; N]);

impl<const N: usize> Bitbag48<N> {
    /// Inserts the value into the bitmap, returning the new duplicity of the value.
    #[inline]
    pub fn insert(&mut self, value: u64) -> usize {
        for (i, bitmap) in self.0.iter_mut().enumerate() {
            if bitmap.insert(value) {
                return i + 1;
            }
        }

        panic!("value {value} exists {N} times already")
    }

    /// Removes the value from the bitmap, returning the new duplicity of the value.
    #[inline]
    pub fn remove(&mut self, value: u64) -> Option<usize> {
        // Remove back to front to remove the value from the bitmap with the highest duplicity first.
        for (i, bitmap) in self.0.iter_mut().rev().enumerate() {
            if bitmap.remove(value) {
                return Some(N - i - 1);
            }
        }

        None
    }

    #[inline]
    pub fn contains(&self, value: u64) -> bool {
        self.0[0].contains(value)
    }

    #[cfg(test)]
    pub fn get(&self, v: u64) -> Option<usize> {
        Some(N - self.0.iter().rev().position(|bitmap| bitmap.contains(v))?)
    }

    /// Returns an iterator over the values and their duplicity.
    pub fn iter(&self) -> impl Iterator<Item = (u64, usize)> + '_ {
        // Only used for debugging for now, so naive impl
        let mut values = self.values().map(|v| (v, 1)).collect::<Vec<_>>();
        for k in 1..N {
            for (value, count) in values.iter_mut() {
                if self.0[k].contains(*value) {
                    *count += 1;
                }
            }
        }
        values.into_iter()
    }

    #[inline]
    pub fn values(&self) -> impl Iterator<Item = u64> + '_ {
        // The first bitmap contains all the values
        self.0[0].iter()
    }
}

impl<const N: usize> BitOrAssign<&Self> for Bitbag48<N> {
    #[inline]
    fn bitor_assign(&mut self, rhs: &Self) {
        // Probably not the most efficient way to do this
        for (value, count) in rhs.iter() {
            for _ in 0..count {
                self.insert(value);
            }
        }
    }
}

impl<const N: usize> SubAssign<&Self> for Bitbag48<N> {
    #[inline]
    fn sub_assign(&mut self, rhs: &Self) {
        // Probably not the most efficient way to do this
        for (value, count) in rhs.iter() {
            for _ in 0..count {
                assert!(self.remove(value).is_some());
            }
        }
    }
}

impl<const N: usize> fmt::Debug for Bitbag48<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<const N: usize> Default for Bitbag48<N> {
    #[inline]
    fn default() -> Self {
        Self(std::array::from_fn(|_| Default::default()))
    }
}

impl<const N: usize> FromIterator<u64> for Bitbag48<N> {
    fn from_iter<I: IntoIterator<Item = u64>>(iter: I) -> Self {
        let mut bitbag = Self::default();
        iter.into_iter().for_each(|id| assert!(bitbag.insert(id) > 0));
        bitbag
    }
}

/// A bitmap with 48-bit values.
/// Basically the same as `roaring::RoaringTreemap` but optimized for 48 bit values.
/// It panics if the upper 16 bits are set which is useful to catch errors.
#[derive(Default, Clone, PartialEq)]
pub(crate) struct Bitmap48(BTreeMap<u16, RoaringBitmap>);

impl fmt::Debug for Bitmap48 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl Bitmap48 {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn insert(&mut self, value: u64) -> bool {
        let (hi, lo) = split(value);
        self.0.entry(hi).or_default().insert(lo)
    }

    pub fn contains(&self, value: u64) -> bool {
        let (hi, lo) = split(value);
        self.0.get(&hi).map_or(false, |map| map.contains(lo))
    }

    pub fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        self.0.iter().flat_map(|(&hi, map)| map.iter().map(move |lo| combine(hi, lo)))
    }

    pub fn remove(&mut self, value: u64) -> bool {
        let (hi, lo) = split(value);
        match self.0.entry(hi) {
            Entry::Vacant(_) => false,
            Entry::Occupied(mut ent) => {
                if ent.get_mut().remove(lo) {
                    if ent.get().is_empty() {
                        ent.remove();
                    }
                    true
                } else {
                    false
                }
            }
        }
    }
}

impl BitOrAssign<Bitmap48> for Bitmap48 {
    fn bitor_assign(&mut self, rhs: Bitmap48) {
        for (key, other_rb) in rhs.0 {
            match self.0.entry(key) {
                Entry::Vacant(ent) => drop(ent.insert(other_rb)),
                Entry::Occupied(mut ent) => *ent.get_mut() |= other_rb,
            }
        }
    }
}

impl BitOrAssign<&Self> for Bitmap48 {
    #[inline]
    fn bitor_assign(&mut self, rhs: &Self) {
        *self |= rhs.clone();
    }
}

impl SubAssign<&Self> for Bitmap48 {
    #[inline]
    fn sub_assign(&mut self, rhs: &Self) {
        *self -= rhs.clone();
    }
}

impl SubAssign<Self> for Bitmap48 {
    fn sub_assign(&mut self, rhs: Self) {
        for (key, rhs_rb) in rhs.0 {
            match self.0.entry(key) {
                Entry::Vacant(_entry) => (),
                Entry::Occupied(mut entry) => {
                    *entry.get_mut() -= rhs_rb;
                    if entry.get().is_empty() {
                        entry.remove_entry();
                    }
                }
            }
        }
    }
}

impl FromIterator<u64> for Bitmap48 {
    fn from_iter<I: IntoIterator<Item = u64>>(iter: I) -> Self {
        let mut bitmap = Self::new();
        iter.into_iter().for_each(|id| {
            bitmap.insert(id);
        });
        bitmap
    }
}

#[inline]
fn split(value: u64) -> (u16, u32) {
    assert_eq!(value >> 48, 0, "upper 16 bits of value must be unused");
    ((value >> 32) as u16, value as u32)
}

fn combine(hi: u16, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

#[cfg(test)]
mod tests {
    use super::Bitbag48;

    #[test]
    fn bitbag_smoke() {
        let mut bag = Bitbag48::<2>::default();
        assert_eq!(bag.insert(1), 1);
        assert_eq!(bag.contains(1), true);
        assert_eq!(bag.get(1), Some(1));

        assert_eq!(bag.insert(1), 2);
        assert_eq!(bag.get(1), Some(2));
        assert_eq!(bag.insert(2), 1);

        assert_eq!(bag.remove(1), Some(1));

        bag |= &Bitbag48::<2>::from_iter([1, 3, 4, 5]);
        assert_eq!(bag.get(1), Some(2));
        assert_eq!(bag.get(3), Some(1));
        assert_eq!(bag.get(4), Some(1));
        assert_eq!(bag.get(5), Some(1));

        bag -= &Bitbag48::<2>::from_iter([1, 2, 3]);
        assert_eq!(bag.get(1), Some(1));
        assert_eq!(bag.get(2), None);
        assert_eq!(bag.get(3), None);
    }

    #[test]
    fn bitbag_setops() {
        let mut bag = Bitbag48::<2>::from_iter([1, 1, 2]);
        assert_eq!(bag.get(1), Some(2));
        assert_eq!(bag.get(2), Some(1));

        bag |= &Bitbag48::<2>::from_iter([2, 3, 3]);
        assert_eq!(bag.get(1), Some(2));
        assert_eq!(bag.get(2), Some(2));
        assert_eq!(bag.get(3), Some(2));

        bag -= &Bitbag48::<2>::from_iter([1, 2, 3]);
        assert_eq!(bag.get(1), Some(1));
        assert_eq!(bag.get(2), Some(1));
        assert_eq!(bag.get(3), Some(1));

        bag -= &Bitbag48::<2>::from_iter([1, 2, 3]);
        assert_eq!(bag.get(1), None);
        assert_eq!(bag.get(2), None);
        assert_eq!(bag.get(3), None);
    }
}
