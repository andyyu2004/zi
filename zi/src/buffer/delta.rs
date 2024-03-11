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
    Full,
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

impl From<Range> for DeltaRange {
    #[inline]
    fn from(v: Range) -> Self {
        Self::Point(v)
    }
}

impl<'a> Delta<'a> {
    #[inline]
    pub fn clear() -> Self {
        Self { range: DeltaRange::Full, text: Cow::Borrowed("") }
    }

    #[inline]
    pub fn delete(range: impl Into<DeltaRange>) -> Self {
        Self { range: range.into(), text: Cow::Borrowed("") }
    }

    #[inline]
    pub fn insert_at(at: impl Into<PointOrChar>, text: impl Into<Cow<'a, str>>) -> Self {
        Self { range: at.into().empty_range(), text: text.into() }
    }

    #[inline]
    pub fn set(text: impl Into<Cow<'a, str>>) -> Self {
        Self { range: DeltaRange::Full, text: text.into() }
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
