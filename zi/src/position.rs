use std::fmt;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    line: Line,
    col: Col,
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

    /// Returns the 0-based (x, y) coordinates of the position
    #[inline]
    pub fn coords(&self) -> (u32, u32) {
        (self.col.0, self.line.0)
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

/// 1-based column index
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Col(u32);

impl fmt::Display for Col {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
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
    pub(crate) fn is_vertical(&self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }

    pub(crate) fn is_horizontal(&self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

#[cfg(test)]
mod tests;
