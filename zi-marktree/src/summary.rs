use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use crop::tree::Metric;

use crate::DebugSet;
use crate::bitbag::Bitbag;
use crate::extent::Extent;

impl Add<Self> for Summary {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: Self) -> Self {
        self += rhs;
        self
    }
}

impl Add<&Self> for Summary {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: &Self) -> Self {
        self += rhs;
        self
    }
}

impl Sub<&Self> for Summary {
    type Output = Self;

    #[inline]
    fn sub(mut self, rhs: &Self) -> Self {
        self -= rhs;
        self
    }
}

impl AddAssign<&Extent> for Summary {
    #[inline]
    fn add_assign(&mut self, rhs: &Extent) {
        self.bytes += rhs.len();
        for key in rhs.keys() {
            // TODO probably is a faster way to do this
            self.ids.insert(key.id());
        }
    }
}

impl SubAssign<&Extent> for Summary {
    #[inline]
    fn sub_assign(&mut self, rhs: &Extent) {
        self.bytes -= rhs.len();
        for key in rhs.keys() {
            // TODO probably is a faster way to do this
            assert!(self.ids.remove(key.id()).is_some());
        }
    }
}

impl AddAssign<Self> for Summary {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self += &rhs;
    }
}

impl AddAssign<&Self> for Summary {
    #[inline]
    fn add_assign(&mut self, rhs: &Self) {
        self.bytes += rhs.bytes;
        self.ids |= &rhs.ids;
    }
}

impl SubAssign<&Self> for Summary {
    #[inline]
    fn sub_assign(&mut self, rhs: &Self) {
        self.bytes -= rhs.bytes;
        self.ids -= &rhs.ids;
    }
}

#[derive(Default, Clone, PartialEq)]
pub(crate) struct Summary {
    /// This needs to be a `bag` not a `set` otherwise the `Sub` operation and `Add` operation will
    /// not be inverses of each other and `crop` assumptions break.
    pub(crate) ids: Bitbag,
    pub(crate) bytes: usize,
}

impl fmt::Debug for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("").field(&self.bytes).field(&DebugSet(|| self.ids.iter())).finish()
    }
}

#[derive(Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) struct ByteMetric(pub(crate) usize);

// NOTE: It would be nice to have the following impl but I don't think it's possible to implement.
// We would need to be able to make arbitrary slices into the `Leaf` which is not possible due to having `Gap`s.
// impl<Id: MTreeId, const N: usize> SlicingMetric<Leaf<T, N>> for ByteMetric {}

impl Metric<Summary> for ByteMetric {
    #[inline]
    fn zero() -> Self {
        Self(0)
    }

    #[inline]
    fn one() -> Self {
        Self(1)
    }

    fn measure(summary: &Summary) -> Self {
        Self(summary.bytes)
    }
}

impl Add<Self> for ByteMetric {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for ByteMetric {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl AddAssign for ByteMetric {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.0 += other.0;
    }
}

impl SubAssign for ByteMetric {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.0 -= other.0;
    }
}

impl Add<usize> for ByteMetric {
    type Output = usize;

    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        self.0 + rhs
    }
}

impl From<ByteMetric> for usize {
    #[inline]
    fn from(metric: ByteMetric) -> Self {
        metric.0
    }
}
