use std::borrow::Cow;
use std::ops;

use crate::{Point, Range};

#[derive(Clone, Debug)]
pub struct Delta<'a> {
    /// The range to replace
    range: DeltaRange,
    /// The text to replace the range with
    text: Cow<'a, str>,
}

#[derive(Clone, Debug)]
pub enum DeltaRange {
    /// The (line, col) range to replace
    Point(Range),
    /// The character index range to replace
    Char(ops::Range<usize>),
}

#[derive(Clone, Copy, Debug)]
pub enum PointOrChar {
    Point(Point),
    Char(usize),
}

impl From<usize> for PointOrChar {
    #[inline]
    fn from(v: usize) -> Self {
        Self::Char(v)
    }
}

impl From<Point> for PointOrChar {
    #[inline]
    fn from(v: Point) -> Self {
        Self::Point(v)
    }
}

impl PointOrChar {
    #[inline]
    fn empty_range(self) -> DeltaRange {
        match self {
            Self::Point(p) => DeltaRange::Point(Range::new(p, p)),
            Self::Char(c) => DeltaRange::Char(c..c),
        }
    }
}

impl From<ops::Range<usize>> for DeltaRange {
    #[inline]
    fn from(v: ops::Range<usize>) -> Self {
        Self::Char(v)
    }
}

impl<R: Into<Range>> From<R> for DeltaRange {
    #[inline]
    fn from(v: R) -> Self {
        Self::Point(v.into())
    }
}

impl<'a> Delta<'a> {
    pub fn new(range: impl Into<DeltaRange>, text: impl Into<Cow<'a, str>>) -> Self {
        Self { range: range.into(), text: text.into() }
    }

    #[inline]
    pub fn delete(range: impl Into<DeltaRange>) -> Self {
        Self::new(range, "")
    }

    #[inline]
    pub fn insert_at(at: impl Into<PointOrChar>, text: impl Into<Cow<'a, str>>) -> Self {
        Self::new(at.into().empty_range(), text)
    }

    #[inline]
    pub fn range(&self) -> DeltaRange {
        self.range.clone()
    }

    #[inline]
    pub fn text(&self) -> &str {
        &self.text
    }
}
