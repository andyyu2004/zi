use std::ops::Bound;

// FIXME these names are not ideal
pub trait BoundExt<T> {
    /// Returns `true` if the range `self..` contains `value`
    fn lt(&self, value: &T) -> bool;

    /// Returns `true` if the range `..self` contains `value`
    fn gt(&self, value: &T) -> bool;
}

impl<T: PartialOrd> BoundExt<T> for Bound<T> {
    #[inline]
    fn lt(&self, value: &T) -> bool {
        match self {
            Bound::Included(bound) => value >= bound,
            Bound::Excluded(bound) => value > bound,
            Bound::Unbounded => true,
        }
    }

    #[inline]
    fn gt(&self, value: &T) -> bool {
        match self {
            Bound::Included(bound) => value > bound,
            Bound::Excluded(bound) => value >= bound,
            Bound::Unbounded => false,
        }
    }
}
