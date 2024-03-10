//! Boring impls converting zi to lsp types

use zi_lsp::lsp_types;

use crate::Point;

impl From<Point> for lsp_types::Position {
    fn from(pos: Point) -> Self {
        lsp_types::Position { line: pos.line().raw(), character: pos.col().raw() }
    }
}

impl From<lsp_types::Position> for Point {
    fn from(pos: lsp_types::Position) -> Self {
        Point::new(pos.line, pos.character)
    }
}
