use std::fmt;
use std::ops::{BitOrAssign, SubAssign};

use croaring::Bitmap;

/// A bitmap with 32-bit values that stores duplicity up to `2`.
// Can be generalized to `N` duplicity, but we can optimize for `N = 2` for now.
#[derive(Clone, PartialEq)]
pub(crate) struct Bitbag([Bitmap; 2]);

impl Bitbag {
    /// Inserts the value into the bitmap, returning the new duplicity of the value.
    #[inline]
    pub fn insert(&mut self, value: u32) -> usize {
        for (i, bitmap) in self.0.iter_mut().enumerate() {
            if bitmap.add_checked(value) {
                return i + 1;
            }
        }

        panic!("value {value} exists 2 times already")
    }

    /// Removes the value from the bitmap, returning the new duplicity of the value.
    #[inline]
    pub fn remove(&mut self, value: u32) -> Option<usize> {
        // Remove back to front to remove the value from the bitmap with the highest duplicity first.
        for (i, bitmap) in self.0.iter_mut().rev().enumerate() {
            if bitmap.remove_checked(value) {
                return Some(1 - i);
            }
        }

        None
    }

    #[inline]
    pub fn contains(&self, value: u32) -> bool {
        self.0[0].contains(value)
    }

    #[cfg(test)]
    pub fn get(&self, v: u32) -> Option<usize> {
        Some(2 - self.0.iter().rev().position(|bitmap| bitmap.contains(v))?)
    }

    /// Returns an iterator over the values and their duplicity.
    pub fn iter(&self) -> impl Iterator<Item = (u32, usize)> + '_ {
        // Only used for debugging for now, so naive impl
        let mut values = self.values().map(|v| (v, 1)).collect::<Vec<_>>();
        for (k, c) in &mut values {
            if self.0[1].contains(*k) {
                *c += 1;
            }
        }
        values.into_iter()
    }

    #[inline]
    pub fn values(&self) -> impl Iterator<Item = u32> + '_ {
        // The first bitmap contains all the values
        self.0[0].iter()
    }
}

impl BitOrAssign<&Self> for Bitbag {
    #[inline]
    fn bitor_assign(&mut self, rhs: &Self) {
        // The items that now have a duplicity of 2 are the ones that:
        // - Had duplicity of 1 in the lhs and also exist in the rhs.
        // - Had duplicity of 2 on either side.
        // Ignore the case where the value ends up having more than 2 duplicity, consider that a caller bug.
        self.0[1] |= (&self.0[0] & &rhs.0[0]) | &rhs.0[1];

        self.0[0] |= &rhs.0[0];
        debug_assert!(self.0[1].is_subset(&self.0[0]));
    }
}

impl SubAssign<&Self> for Bitbag {
    #[inline]
    fn sub_assign(&mut self, rhs: &Self) {
        self.0[0] -= (&rhs.0[0] - &self.0[1]) | &rhs.0[1];
        self.0[1] -= &rhs.0[0];
        debug_assert!(self.0[1].is_subset(&self.0[0]));
    }
}

impl fmt::Debug for Bitbag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl Default for Bitbag {
    #[inline]
    fn default() -> Self {
        Self(std::array::from_fn(|_| Default::default()))
    }
}

impl FromIterator<u32> for Bitbag {
    #[inline]
    fn from_iter<I: IntoIterator<Item = u32>>(iter: I) -> Self {
        let mut bitbag = Self::default();
        iter.into_iter().for_each(|id| {
            let k = bitbag.insert(id);
            debug_assert!(k > 0);
        });
        bitbag
    }
}

#[cfg(test)]
mod tests {
    use super::Bitbag;

    #[test]
    fn bitbag_smoke() {
        let mut bag = Bitbag::default();
        assert_eq!(bag.insert(1), 1);
        assert!(bag.contains(1));
        assert_eq!(bag.get(1), Some(1));

        assert_eq!(bag.insert(1), 2);
        assert_eq!(bag.get(1), Some(2));
        assert_eq!(bag.insert(2), 1);

        assert_eq!(bag.remove(1), Some(1));

        bag |= &Bitbag::from_iter([1, 3, 4, 5]);
        assert_eq!(bag.get(1), Some(2));
        assert_eq!(bag.get(3), Some(1));
        assert_eq!(bag.get(4), Some(1));
        assert_eq!(bag.get(5), Some(1));

        bag -= &Bitbag::from_iter([1, 2, 3]);
        assert_eq!(bag.get(1), Some(1));
        assert_eq!(bag.get(2), None);
        assert_eq!(bag.get(3), None);
    }

    #[test]
    fn bitbag_setops() {
        let mut bag = Bitbag::from_iter([1, 1, 2]);
        assert_eq!(bag.get(1), Some(2));
        assert_eq!(bag.get(2), Some(1));

        bag |= &Bitbag::from_iter([2, 3, 3]);
        assert_eq!(bag.get(1), Some(2));
        assert_eq!(bag.get(2), Some(2));
        assert_eq!(bag.get(3), Some(2));

        bag -= &Bitbag::from_iter([1, 2, 3]);
        assert_eq!(bag.get(1), Some(1));
        assert_eq!(bag.get(2), Some(1));
        assert_eq!(bag.get(3), Some(1));

        bag -= &Bitbag::from_iter([1, 2, 3]);
        assert_eq!(bag.get(1), None);
        assert_eq!(bag.get(2), None);
        assert_eq!(bag.get(3), None);

        let mut bag = Bitbag::from_iter([1, 1, 2]);
        bag -= &Bitbag::from_iter([1, 1, 2]);
        assert_eq!(bag.get(1), None);
        assert_eq!(bag.get(2), None);
    }
}
