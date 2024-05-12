#[cfg(feature = "tree-sitter")]
mod tree_sitter_impls;

use std::cmp::Ordering;
use std::collections::VecDeque;
use std::iter::Peekable;
use std::ops::{Add, Bound, RangeBounds, Sub};
use std::str::FromStr;
use std::{fmt, ops};

use stdx::merge::Merge;
use tui::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl Size {
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

impl From<Rect> for Size {
    #[inline]
    fn from(rect: Rect) -> Self {
        Self::new(rect.width, rect.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Offset {
    pub line: usize,
    pub col: usize,
}

impl Offset {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }

    pub fn new_line(line: usize) -> Self {
        Self::new(line, 0)
    }

    pub fn new_col(col: usize) -> Self {
        Self::new(0, col)
    }
}

impl fmt::Display for Offset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

impl PartialEq<(usize, usize)> for Offset {
    #[inline]
    fn eq(&self, &(line, col): &(usize, usize)) -> bool {
        self.line == line && self.col == col
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PointRange {
    // TODO these should maybe each be Bound<Point> to be more flexible
    /// The start of the range (inclusive)
    start: Point,
    /// The end of the range (exclusive)
    end: Point,
}

impl RangeBounds<Point> for PointRange {
    #[inline]
    fn start_bound(&self) -> Bound<&Point> {
        Bound::Included(&self.start)
    }

    #[inline]
    fn end_bound(&self) -> Bound<&Point> {
        Bound::Excluded(&self.end)
    }
}

impl PointRange {
    #[inline]
    pub fn new(start: impl Into<Point>, end: impl Into<Point>) -> Self {
        let start = start.into();
        let end = end.into();
        assert!(start <= end, "start must be less than end: {} !<= {}", start, end);
        Self { start, end }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    #[inline]
    pub fn intersects(&self, other: &PointRange) -> bool {
        self.start < other.end && self.end > other.start
    }

    #[inline]
    pub fn is_single_line(&self) -> bool {
        self.start.line == self.end.line
    }

    #[inline]
    pub fn start(&self) -> Point {
        self.start
    }

    #[inline]
    pub fn end(&self) -> Point {
        self.end
    }

    #[inline]
    pub fn is_subrange_of(&self, other: impl Into<PointRange>) -> bool {
        let other = other.into();
        other.start <= self.start && self.end <= other.end
    }

    /// Split the range into three segments: before, inside, and after the other range.
    /// Panics if the ranges do not intersect.
    #[inline]
    pub fn segments(&self, other: &PointRange) -> (PointRange, PointRange, PointRange) {
        assert!(self.intersects(other), "ranges must intersect");

        let start = self.start.min(other.start);
        let end = self.end.max(other.end);
        let inside_start = self.start.max(other.start);
        let inside_end = self.end.min(other.end);

        let before = PointRange::new(start, inside_start);
        let inside = PointRange::new(inside_start, inside_end);
        let after = PointRange::new(inside_end, end);
        (before, inside, after)
    }
}

impl FromStr for PointRange {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (start, end) = s.split_once("..").ok_or_else(|| {
            anyhow::anyhow!("invalid range: {s} (expected `<line>:<col>..<line>:<col>`)")
        })?;
        Ok(Self::new(start.parse::<Point>()?, end.parse::<Point>()?))
    }
}

impl fmt::Display for PointRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl fmt::Debug for PointRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl Sub<Offset> for PointRange {
    type Output = Self;

    #[inline]
    fn sub(self, offset: Offset) -> Self {
        Self::new(self.start - offset, self.end - offset)
    }
}

impl From<PointRange> for ops::Range<Point> {
    fn from(val: PointRange) -> Self {
        val.start..val.end
    }
}

impl From<PointRange> for ops::Range<(usize, usize)> {
    #[inline]
    fn from(r: PointRange) -> Self {
        r.start.into()..r.end.into()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointOrByte {
    Point(Point),
    Byte(usize),
}

impl PointOrByte {
    #[inline]
    pub fn try_into_point(self) -> Result<Point, Self> {
        if let Self::Point(v) = self { Ok(v) } else { Err(self) }
    }

    #[inline]
    pub fn try_into_byte(self) -> Result<usize, Self> {
        if let Self::Byte(v) = self { Ok(v) } else { Err(self) }
    }
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

impl From<(usize, usize)> for PointOrByte {
    #[inline]
    fn from(v: (usize, usize)) -> Self {
        Self::Point(v.into())
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    line: Line,
    col: Col,
}

impl Add<Offset> for Point {
    type Output = Self;

    #[inline]
    fn add(self, offset: Offset) -> Self {
        Self::new(self.line + offset.line, self.col + offset.col)
    }
}

impl Sub<Offset> for Point {
    type Output = Self;

    #[inline]
    fn sub(self, offset: Offset) -> Self {
        Self::new(self.line - offset.line, self.col - offset.col)
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

impl fmt::Debug for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl FromStr for Point {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (line, col) = s
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("invalid position: {s} (expected `<line>:<col>`)"))?;
        Ok(Self::new(line.parse::<usize>()?, col.parse::<usize>()?))
    }
}

impl From<Point> for (usize, usize) {
    #[inline]
    fn from(val: Point) -> Self {
        (val.line, val.col)
    }
}

impl From<(usize, usize)> for Point {
    #[inline]
    fn from((line, col): (usize, usize)) -> Self {
        Self { line, col }
    }
}

impl PartialEq<(usize, usize)> for Point {
    #[inline]
    fn eq(&self, &(line, col): &(usize, usize)) -> bool {
        self.line == line && self.col == col
    }
}

impl Point {
    #[inline]
    pub fn new(line: Line, col: Col) -> Self {
        Self { line, col }
    }

    #[inline]
    pub fn line(&self) -> Line {
        self.line
    }

    #[inline]
    pub fn col(&self) -> Col {
        self.col
    }

    #[inline]
    pub fn left(self, amt: usize) -> Self {
        Self::new(self.line, self.col.saturating_sub(amt))
    }

    #[inline]
    pub fn up(self, amt: usize) -> Self {
        Self::new(self.line.saturating_sub(amt), self.col)
    }

    #[inline]
    pub fn down(self, amt: usize) -> Self {
        Self::new(self.line.saturating_add(amt), self.col)
    }

    #[inline]
    pub fn right(self, amt: usize) -> Self {
        Self::new(self.line, self.col.saturating_add(amt))
    }

    pub fn with_line(self, line: Line) -> Self {
        Self::new(line, self.col)
    }

    #[inline]
    pub fn with_col(self, col: Col) -> Self {
        Self::new(self.line, col)
    }
}

pub type Line = usize;

pub type Col = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    #[inline]
    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }

    #[inline]
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

impl From<Direction> for tui::Direction {
    #[inline]
    fn from(value: Direction) -> Self {
        match value {
            Direction::Left | Direction::Right => tui::Direction::Horizontal,
            Direction::Up | Direction::Down => tui::Direction::Vertical,
        }
    }
}

/// An iterator that merges two iterators over ([`Range`], T: [`Merge`]), prioritizing the second iterator on overlap (as per [`Merge`])
pub struct RangeMergeIter<I: Iterator, J: Iterator, T> {
    xs: Peekable<I>,
    ys: Peekable<J>,
    /// Stack of buffered ranges that should be immediately yielded
    buffer: Vec<(PointRange, T)>,
    xs_buffer: VecDeque<(PointRange, T)>,
    ys_buffer: VecDeque<(PointRange, T)>,
}

impl<I, J, T> RangeMergeIter<I, J, T>
where
    I: Iterator<Item = (PointRange, T)>,
    J: Iterator<Item = (PointRange, T)>,
{
    pub fn new(xs: I, ys: J) -> Self {
        Self {
            xs: xs.peekable(),
            ys: ys.peekable(),
            buffer: Default::default(),
            xs_buffer: Default::default(),
            ys_buffer: Default::default(),
        }
    }
}

impl<I, J, T> Iterator for RangeMergeIter<I, J, T>
where
    T: Merge + Copy,
    I: Iterator<Item = (PointRange, T)>,
    J: Iterator<Item = (PointRange, T)>,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        macro_rules! peek {
            (x) => {
                self.xs_buffer.front().or_else(|| self.xs.peek())
            };
            (y) => {
                self.ys_buffer.front().or_else(|| self.ys.peek())
            };
            (x, y) => {
                (peek!(x), peek!(y))
            };
        }

        macro_rules! next {
            (x) => {
                self.xs_buffer.pop_front().or_else(|| self.xs.next())
            };
            (y) => {
                self.ys_buffer.pop_front().or_else(|| self.ys.next())
            };
            (x, y) => {
                (next!(x), next!(y))
            };
        }

        loop {
            while let Some((range, style)) = self.buffer.pop() {
                if !range.is_empty() {
                    return Some((range, style));
                }
            }

            let ((xr, x), (yr, y)) = match peek!(x, y) {
                (None, None) => return None,
                (Some(_), None) => return next!(x),
                (None, Some(_)) => return next!(y),
                (Some(&x), Some(&y)) => (x, y),
            };

            // It's generally not possible to merge non-single-line ranges.
            // Given ranges 0:0:5:2 and 0:3:0:5, it's ambiguous what the output should be.
            // In particular, it's ambiguous where the first highlight should end for each line.
            assert!(
                xr.is_single_line() && yr.is_single_line(),
                "can only merge single-line ranges: {xr} {yr}"
            );

            if xr.intersects(&yr) {
                next!(x, y);

                let (before, inside, after) = xr.segments(&yr);

                if !after.is_empty() {
                    if xr.end < yr.end {
                        self.ys_buffer.push_back((after, y));
                    } else {
                        self.xs_buffer.push_back((after, x));
                    }
                }

                match xr.start.cmp(&yr.start) {
                    Ordering::Less => {
                        self.buffer.push((inside, x.merge(y)));
                        self.buffer.push((before, x));
                    }
                    Ordering::Equal => {
                        assert!(before.is_empty());
                        return Some((inside, x.merge(y)));
                    }
                    Ordering::Greater => {
                        self.buffer.push((inside, x.merge(y)));
                        self.buffer.push((before, y));
                    }
                }
            } else if xr.start < yr.start {
                return next!(x);
            } else {
                return next!(y);
            }
        }
    }
}

#[cfg(test)]
mod tests;
