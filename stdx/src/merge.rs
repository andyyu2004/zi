/// A right-biased merge operation.
pub trait Merge {
    fn merge(self, other: Self) -> Self;
}
