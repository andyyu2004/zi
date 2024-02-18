use std::num::NonZeroU16;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    line: Line,
    col: Col,
}

impl Position {
    #[inline]
    pub fn new(row: Line, col: Col) -> Self {
        Self { line: row, col }
    }

    #[inline]
    pub fn row(&self) -> Line {
        self.line
    }

    #[inline]
    pub fn col(&self) -> Col {
        self.col
    }

    /// Returns the 0-based (x, y) coordinates of the position
    #[inline]
    pub fn coords(&self) -> (u16, u16) {
        (self.col.0, self.line.0.get() - 1)
    }

    #[inline]
    pub fn left(self, amt: u16) -> Self {
        Self::new(self.line, self.col.left(amt))
    }

    #[inline]
    pub fn up(self, amt: u16) -> Self {
        Self::new(self.line.left(amt), self.col)
    }

    #[inline]
    pub fn down(self, amt: u16) -> Self {
        Self::new(self.line.right(amt), self.col)
    }

    #[inline]
    pub fn right(self, amt: u16) -> Self {
        Self::new(self.line, self.col.right(amt))
    }
}

/// 1-based line number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Line(NonZeroU16);

impl Line {
    #[inline]
    pub fn left(self, amt: u16) -> Self {
        match NonZeroU16::new(self.0.get().saturating_sub(amt)) {
            Some(n) => Self(n),
            None => self,
        }
    }

    #[inline]
    pub fn right(self, amt: u16) -> Self {
        Self(unsafe { NonZeroU16::new_unchecked(self.0.get().saturating_add(amt)) })
    }
}

impl Default for Line {
    fn default() -> Self {
        // SAFETY: 1 is non-zero
        Self(unsafe { NonZeroU16::new(1).unwrap_unchecked() })
    }
}

/// 1-based column index
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Col(u16);

impl Col {
    #[inline]
    pub fn left(self, amt: u16) -> Self {
        Self(self.0.saturating_sub(amt))
    }

    #[inline]
    pub fn right(self, amt: u16) -> Self {
        Self(self.0.saturating_add(amt))
    }
}

#[cfg(test)]
mod tests;
