use std::fmt;
use std::ops::Add;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Offset {
    pub line: u32,
    pub col: u32,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    line: Line,
    col: Col,
}

impl Add<Offset> for Position {
    type Output = Self;

    fn add(self, offset: Offset) -> Self {
        Self::new(self.line + offset.line, self.col + offset.col)
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

impl<L, C> From<(L, C)> for Position
where
    Col: From<C>,
    Line: From<L>,
{
    fn from((line, col): (L, C)) -> Self {
        Self { line: Line::from(line), col: Col::from(col) }
    }
}

impl PartialEq<(u32, u32)> for Position {
    fn eq(&self, &(line, col): &(u32, u32)) -> bool {
        self.line.0 == line && self.col.0 == col
    }
}

impl Position {
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

    fn add(self, amt: u32) -> Self {
        Self(self.0 + amt)
    }
}

impl From<u32> for Line {
    fn from(n: u32) -> Self {
        Self(n)
    }
}

impl From<i32> for Line {
    fn from(n: i32) -> Self {
        assert!(n >= 0, "Line number must be non-negative");
        Self(n as u32)
    }
}

impl From<usize> for Line {
    fn from(n: usize) -> Self {
        assert!(n < u32::MAX as usize, "Line number must be less than u32::MAX");
        Self(n as u32)
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
    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }

    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

#[cfg(test)]
mod tests;
