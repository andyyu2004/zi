use std::borrow::Cow;
use std::ops;
use std::ops::RangeBounds;

use super::Text;
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
    /// The point range to replace
    Point(Range),
    /// The byte range to replace
    Byte(ops::Range<usize>),
}

impl DeltaRange {
    #[inline]
    pub fn start(&self) -> PointOrByte {
        match self {
            Self::Point(r) => PointOrByte::Point(r.start()),
            Self::Byte(r) => PointOrByte::Byte(r.start),
        }
    }

    #[inline]
    pub fn end(&self) -> PointOrByte {
        match self {
            Self::Point(r) => PointOrByte::Point(r.end()),
            Self::Byte(r) => PointOrByte::Byte(r.end),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Point(r) => r.is_empty(),
            Self::Byte(r) => r.is_empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointOrByte {
    Point(Point),
    Byte(usize),
}

impl From<usize> for PointOrByte {
    #[inline]
    fn from(v: usize) -> Self {
        Self::Byte(v)
    }
}

impl From<Point> for PointOrByte {
    #[inline]
    fn from(v: Point) -> Self {
        Self::Point(v)
    }
}

impl PointOrByte {
    #[inline]
    fn empty_range(self) -> DeltaRange {
        match self {
            Self::Point(p) => DeltaRange::Point(Range::new(p, p)),
            Self::Byte(c) => DeltaRange::Byte(c..c),
        }
    }
}

impl From<ops::Range<usize>> for DeltaRange {
    #[inline]
    fn from(v: ops::Range<usize>) -> Self {
        Self::Byte(v)
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
    pub fn insert_at(at: impl Into<PointOrByte>, text: impl Into<Cow<'a, str>>) -> Self {
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

    #[inline]
    pub fn to_owned(&self) -> Delta<'static> {
        Delta {
            range: match &self.range {
                DeltaRange::Point(r) => DeltaRange::Point(*r),
                DeltaRange::Byte(r) => DeltaRange::Byte(r.clone()),
            },
            text: Cow::Owned(self.text.to_string()),
        }
    }

    #[inline]
    pub fn is_identity(&self) -> bool {
        self.text.is_empty() && self.range.is_empty()
    }

    /// Apply the delta to the text and return the inverse delta
    pub(crate) fn apply(&self, text: &mut impl TextReplace) -> Delta<'static> {
        let byte_range = text.delta_to_byte_range(self);
        let start = byte_range.start;
        let deleted_text = text.byte_slice(byte_range.clone()).to_string();
        text.replace(byte_range, self.text());
        Delta::new(start..start + self.text.len(), deleted_text)
    }
}

// HACK trait, do not expose
pub(crate) trait TextReplace: Text {
    fn replace(&mut self, byte_range: impl RangeBounds<usize>, text: &str);
}

impl TextReplace for String {
    fn replace(&mut self, byte_range: impl RangeBounds<usize>, text: &str) {
        self.replace_range(byte_range, text);
    }
}

impl TextReplace for crop::Rope {
    fn replace(&mut self, byte_range: impl RangeBounds<usize>, text: &str) {
        self.replace(byte_range, text);
    }
}
