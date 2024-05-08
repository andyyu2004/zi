use crate::{Point, PointRange};

impl From<tree_sitter::Range> for PointRange {
    #[inline]
    fn from(range: tree_sitter::Range) -> Self {
        Self::new(range.start_point, range.end_point)
    }
}

impl From<Point> for tree_sitter::Point {
    #[inline]
    fn from(point: Point) -> Self {
        Self { row: point.line, column: point.col }
    }
}

impl From<tree_sitter::Point> for Point {
    #[inline]
    fn from(point: tree_sitter::Point) -> Self {
        Self::new(point.row, point.column)
    }
}
