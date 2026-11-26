use std::fmt;

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub struct Flags: u32 {
        const BIAS_LEFT = 1 << 0;
        // If the key is part of a range pair.
        const RANGE = 1 << 1;
        /// The end of a range pair.
        const END = 1 << 2;
    }
}

impl fmt::Debug for Flags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Key packs an id into the low `ID_BITS` bits and the flags into the bits above them.
#[derive(Clone, Copy)]
pub(super) struct Key(u64);

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("").field(&self.id()).field(&self.flags()).finish()
    }
}

impl Key {
    const ID_BITS: u32 = super::ID_BITS;
    const ID_MASK: u64 = super::MAX_ID;

    pub fn new(id: u64, flags: Flags) -> Self {
        debug_assert!(id <= Self::ID_MASK, "id {id} does not fit in {} bits", Self::ID_BITS);
        let flag_bits = u64::from(flags.bits()) << Self::ID_BITS;
        Self(id | flag_bits)
    }

    #[inline]
    pub fn id(self) -> u64 {
        self.0 & Self::ID_MASK
    }

    #[inline]
    pub fn flags(self) -> Flags {
        let flag_bits = self.0 >> Self::ID_BITS;
        Flags::from_bits(flag_bits as u32).unwrap()
    }

    #[inline]
    pub fn is_end(self) -> bool {
        self.flags().contains(Flags::END)
    }

    #[inline]
    pub fn is_range(self) -> bool {
        self.flags().contains(Flags::RANGE)
    }

    #[inline]
    pub fn into_raw(self) -> u64 {
        self.0
    }

    #[inline]
    pub fn from_raw(id: u64) -> Self {
        Self(id)
    }
}

#[cfg(test)]
mod tests {
    use super::{Flags, Key};
    use crate::MAX_ID;

    #[test]
    fn key_roundtrip() {
        for id in [0, 1, u64::from(u32::MAX), u64::from(u32::MAX) + 1, MAX_ID - 1, MAX_ID] {
            for bits in 0..=Flags::all().bits() {
                let flags = Flags::from_bits(bits).unwrap();
                let key = Key::from_raw(Key::new(id, flags).into_raw());
                assert_eq!(key.id(), id);
                assert_eq!(key.flags(), flags);
            }
        }
    }
}
