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
        Self { row: point.line.0 as usize, column: point.col.0 as usize }
    }
}

impl From<tree_sitter::Point> for Point {
    #[inline]
    fn from(point: tree_sitter::Point) -> Self {
        Self::new(point.row as u32, point.column as u32)
    }
}
