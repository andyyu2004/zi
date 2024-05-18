//! Boring impls converting zi to lsp types

use zi_core::PointRange;
use zi_lsp::lsp_types;
use zi_text::{Delta, Deltas, Text};

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

pub fn deltas(
    text: impl Text,
    edits: impl IntoIterator<Item = lsp_types::TextEdit>,
) -> Deltas<'static> {
    Deltas::new(edits.into_iter().map(|edit| {
        let range = text.point_range_to_byte_range(edit.range.conv());
        Delta::new(range, edit.new_text)
    }))
}

impl Conv for lsp_types::Range {
    type Converted = PointRange;

    #[inline]
    fn conv(self) -> Self::Converted {
        PointRange::new(self.start.conv(), self.end.conv())
    }
}
