//! Boring impls converting zi to lsp types

use zi_lsp::lsp_types;

use crate::Point;

pub(crate) trait Conv {
    type Converted;

    fn conv(self) -> Self::Converted;
}

impl Conv for Point {
    type Converted = lsp_types::Position;

    #[inline]
    fn conv(self) -> Self::Converted {
        lsp_types::Position { line: self.line() as u32, character: self.col() as u32 }
    }
}

impl Conv for lsp_types::Position {
    type Converted = Point;

    #[inline]
    fn conv(self) -> Self::Converted {
        Point::new(self.line as usize, self.character as usize)
    }
}
