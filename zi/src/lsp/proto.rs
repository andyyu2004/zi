//! Boring impls converting zi to lsp types

use zi_lsp::lsp_types;

use crate::Point;

pub(crate) trait Conv {
    type Converted;

    fn conv(self) -> Self::Converted;
}

impl Conv for Point {
    type Converted = lsp_types::Position;

    fn conv(self) -> Self::Converted {
        lsp_types::Position { line: self.line().raw(), character: self.col().raw() }
    }
}

impl Conv for lsp_types::Position {
    type Converted = Point;

    fn conv(self) -> Self::Converted {
        Point::new(self.line, self.character)
    }
}

