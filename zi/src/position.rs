use std::cmp::Ordering;
use std::collections::VecDeque;
use std::fmt;
use std::iter::Peekable;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::str::FromStr;

use stdx::merge::Merge;
use tui::Rect;

use crate::BufferId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
    pub line: u32,
    pub col: u32,
}

impl Offset {
    pub fn new(line: u32, col: u32) -> Self {
        Self { line, col }
    }

    pub fn new_line(line: u32) -> Self {
        Self::new(line, 0)
    }

    pub fn new_col(col: u32) -> Self {
        Self::new(0, col)
    }
}

impl fmt::Display for Offset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

impl PartialEq<(u32, u32)> for Offset {
    #[inline]
    fn eq(&self, &(line, col): &(u32, u32)) -> bool {
        self.line == line && self.col == col
    }
}

pub struct Location {
    pub buffer: BufferId,
    pub range: Range,
}

impl Location {
    pub fn new(buffer: BufferId, range: Range) -> Self {
        Self { buffer, range }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Range {
    start: Point,
    end: Point,
}

impl Range {
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
    pub fn intersects(&self, other: &Range) -> bool {
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

    /// Split the range into three segments: before, inside, and after the other range.
    /// Panics if the ranges do not intersect.
    #[inline]
    pub fn segments(&self, other: &Range) -> (Range, Range, Range) {
        assert!(self.intersects(other), "ranges must intersect");

        let start = self.start.min(other.start);
        let end = self.end.max(other.end);
        let inside_start = self.start.max(other.start);
        let inside_end = self.end.min(other.end);

        let before = Range::new(start, inside_start);
        let inside = Range::new(inside_start, inside_end);
        let after = Range::new(inside_end, end);
        debug_assert!(!inside.is_empty(), "they intersected so inside can't be empty");
        (before, inside, after)
    }
}

impl FromStr for Range {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (start, end) = s.split_once("..").ok_or_else(|| {
            anyhow::anyhow!("invalid range: {s} (expected `<line>:<col>..<line>:<col>`)")
        })?;
        Ok(Self::new(start.parse::<Point>()?, end.parse::<Point>()?))
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl fmt::Debug for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl Sub<Offset> for Range {
    type Output = Self;

    #[inline]
    fn sub(self, offset: Offset) -> Self {
        Self::new(self.start - offset, self.end - offset)
    }
}

impl From<tree_sitter::Range> for Range {
    #[inline]
    fn from(range: tree_sitter::Range) -> Self {
        Self::new(range.start_point, range.end_point)
    }
}

impl From<Range> for std::ops::Range<Point> {
    fn from(val: Range) -> Self {
        val.start..val.end
    }
}

impl From<Range> for std::ops::Range<(usize, usize)> {
    #[inline]
    fn from(r: Range) -> Self {
        r.start.into()..r.end.into()
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
        Ok(Self::new(line.parse::<u32>()?, col.parse::<u32>()?))
    }
}

impl From<tree_sitter::Point> for Point {
    #[inline]
    fn from(point: tree_sitter::Point) -> Self {
        Self::new(point.row as u32, point.column as u32)
    }
}

impl From<Point> for (u32, u32) {
    #[inline]
    fn from(val: Point) -> Self {
        (val.line.0, val.col.0)
    }
}

impl From<Point> for (usize, usize) {
    #[inline]
    fn from(val: Point) -> Self {
        (val.line.idx(), val.col.idx())
    }
}

impl<L, C> From<(L, C)> for Point
where
    Col: From<C>,
    Line: From<L>,
{
    #[inline]
    fn from((line, col): (L, C)) -> Self {
        Self { line: Line::from(line), col: Col::from(col) }
    }
}

impl PartialEq<(u32, u32)> for Point {
    #[inline]
    fn eq(&self, &(line, col): &(u32, u32)) -> bool {
        self.line.0 == line && self.col.0 == col
    }
}

impl Point {
    #[inline]
    pub fn new(line: impl Into<Line>, col: impl Into<Col>) -> Self {
        Self { line: line.into(), col: col.into() }
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
    pub fn left(self, amt: u32) -> Self {
        Self::new(self.line, self.col.left(amt))
    }

    #[inline]
    pub fn up(self, amt: u32) -> Self {
        Self::new(self.line.up(amt), self.col)
    }

    #[inline]
    pub fn down(self, amt: u32) -> Self {
        Self::new(self.line.down(amt), self.col)
    }

    #[inline]
    pub fn right(self, amt: u32) -> Self {
        Self::new(self.line, self.col.right(amt))
    }

    pub fn with_line(self, line: impl Into<Line>) -> Self {
        Self::new(line, self.col)
    }

    #[inline]
    pub fn with_col(self, col: impl Into<Col>) -> Self {
        Self::new(self.line, col)
    }
}

/// 0-based line number
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Line(u32);

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add<u32> for Line {
    type Output = Self;

    #[inline]
    fn add(self, amt: u32) -> Self {
        Self(self.0.saturating_add(amt))
    }
}

impl Add<usize> for Line {
    type Output = Self;

    #[inline]
    fn add(self, amt: usize) -> Self {
        self + amt as u32
    }
}

impl AddAssign<usize> for Line {
    #[inline]
    fn add_assign(&mut self, amt: usize) {
        *self = *self + amt;
    }
}

impl SubAssign<usize> for Line {
    #[inline]
    fn sub_assign(&mut self, amt: usize) {
        *self = *self - amt;
    }
}

impl Sub<usize> for Line {
    type Output = Self;

    #[inline]
    fn sub(self, amt: usize) -> Self {
        self - amt as u32
    }
}

impl Sub<u32> for Line {
    type Output = Self;

    #[inline]
    fn sub(self, amt: u32) -> Self {
        Self(self.0.saturating_sub(amt))
    }
}

impl From<u32> for Line {
    #[inline]
    fn from(n: u32) -> Self {
        Self(n)
    }
}

impl From<i32> for Line {
    #[inline]
    fn from(n: i32) -> Self {
        assert!(n >= 0, "Line number must be non-negative");
        Self(n as u32)
    }
}

impl From<usize> for Line {
    #[inline]
    fn from(n: usize) -> Self {
        assert!(n < u32::MAX as usize, "Line number must be less than u32::MAX");
        Self(n as u32)
    }
}

impl PartialEq<usize> for Line {
    #[inline]
    fn eq(&self, &other: &usize) -> bool {
        self.0 as usize == other
    }
}

impl PartialOrd<usize> for Line {
    #[inline]
    fn partial_cmp(&self, &other: &usize) -> Option<Ordering> {
        self.partial_cmp(&Line::from(other))
    }
}

impl Line {
    #[inline]
    pub fn up(self, amt: u32) -> Self {
        Self(self.0.saturating_sub(amt))
    }

    #[inline]
    pub fn down(self, amt: u32) -> Self {
        Self(self.0.saturating_add(amt))
    }

    #[inline]
    pub fn idx(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// 1-based column index in characters (not bytes)
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Col(u32);

impl fmt::Display for Col {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add<u32> for Col {
    type Output = Self;

    fn add(self, amt: u32) -> Self {
        Self(self.0 + amt)
    }
}

impl Sub<u32> for Col {
    type Output = Self;

    fn sub(self, amt: u32) -> Self {
        Self(self.0 - amt)
    }
}

impl From<i32> for Col {
    fn from(n: i32) -> Self {
        assert!(n >= 0, "Column number must be non-negative");
        Self(n as u32)
    }
}

impl From<u32> for Col {
    fn from(n: u32) -> Self {
        Self(n)
    }
}

impl From<usize> for Col {
    fn from(n: usize) -> Self {
        assert!(n < u32::MAX as usize, "Column number must be less than u32::MAX");
        Self(n as u32)
    }
}

impl PartialEq<usize> for Col {
    #[inline]
    fn eq(&self, &other: &usize) -> bool {
        self.0 as usize == other
    }
}

impl PartialOrd<usize> for Col {
    #[inline]
    fn partial_cmp(&self, &other: &usize) -> Option<Ordering> {
        self.partial_cmp(&Col::from(other))
    }
}

impl Col {
    #[inline]
    pub fn left(self, amt: u32) -> Self {
        Self(self.0.saturating_sub(amt))
    }

    #[inline]
    pub fn right(self, amt: u32) -> Self {
        Self(self.0.saturating_add(amt))
    }

    #[inline]
    pub fn idx(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
}

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
pub(crate) struct RangeMergeIter<I: Iterator, J: Iterator, T> {
    xs: Peekable<I>,
    ys: Peekable<J>,
    /// Stack of buffered ranges that should be immediately yielded
    buffer: Vec<(Range, T)>,
    xs_buffer: VecDeque<(Range, T)>,
    ys_buffer: VecDeque<(Range, T)>,
}

impl<I, J, T> RangeMergeIter<I, J, T>
where
    I: Iterator<Item = (Range, T)>,
    J: Iterator<Item = (Range, T)>,
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
    I: Iterator<Item = (Range, T)>,
    J: Iterator<Item = (Range, T)>,
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

        if let Some((range, style)) = self.buffer.pop() {
            return Some((range, style));
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
            debug_assert!(!inside.is_empty());

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
                    Some((before, x))
                }
                Ordering::Equal => {
                    assert!(before.is_empty());
                    Some((inside, x.merge(y)))
                }
                Ordering::Greater => {
                    self.buffer.push((inside, x.merge(y)));
                    Some((before, y))
                }
            }
        } else if xr.start < yr.start {
            next!(x)
        } else {
            next!(y)
        }
    }
}

#[cfg(test)]
mod tests;
