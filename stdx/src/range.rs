use std::ops;

pub trait RangeExt<T> {
    fn intersects(&self, other: &Self) -> bool;
}

impl<T: PartialOrd> RangeExt<T> for ops::Range<T> {
    fn intersects(&self, other: &Self) -> bool {
        self.start <= other.end && other.start <= self.end
    }
}
