/// A count typed before a command. Like vim's, it stops growing at 999999999.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Count(u32);

impl Count {
    const MAX: u32 = 999_999_999;

    /// The count after typing `digit`, appended to `count` if one was typed already
    pub(crate) fn push_digit(count: Option<Self>, digit: u8) -> Self {
        let Self(n) = count.unwrap_or(Self(0));
        Self(n.saturating_mul(10).saturating_add(u32::from(digit)).min(Self::MAX))
    }

    pub(crate) fn saturating_mul(self, Self(other): Self) -> Self {
        let Self(n) = self;
        Self(n.saturating_mul(other).min(Self::MAX))
    }

    pub(crate) fn get(self) -> usize {
        let Self(n) = self;
        n as usize
    }
}

#[cfg(test)]
mod tests {
    use super::Count;

    #[test]
    fn push_digit() {
        let count =
            [1, 2, 3].into_iter().fold(None, |count, digit| Some(Count::push_digit(count, digit)));
        assert_eq!(count.map(Count::get), Some(123));
    }

    #[test]
    fn push_digit_stops_at_max() {
        let count = (0..20).fold(None, |count, _| Some(Count::push_digit(count, 9)));
        assert_eq!(count.map(Count::get), Some(999_999_999));
    }

    #[test]
    fn saturating_mul_stops_at_max() {
        let count = Count::push_digit(None, 9);
        let big = (0..9).fold(None, |count, _| Some(Count::push_digit(count, 9))).unwrap();
        assert_eq!(big.saturating_mul(count).get(), 999_999_999);
        assert_eq!(count.saturating_mul(count).get(), 81);
    }
}
