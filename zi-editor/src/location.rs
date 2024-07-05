use std::fmt;

use zi_core::Point;

use crate::BufferId;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
    pub buf: BufferId,
    pub point: Point,
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}:{}", self.buf, self.point)
    }
}

impl Location {
    pub fn new(buf: BufferId, point: impl Into<Point>) -> Self {
        Self { buf, point: point.into() }
    }
}
