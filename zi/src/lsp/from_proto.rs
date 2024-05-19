//! Boring impls converting zi to lsp types

use zi_core::PointRange;
use zi_lsp::{lsp_types, PositionEncoding};
use zi_text::{Delta, Deltas, Text};

use crate::Point;

pub fn point(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    point: lsp_types::Position,
) -> Point {
    match encoding {
        PositionEncoding::Utf8 => Point::new(point.line as usize, point.character as usize),
        PositionEncoding::Utf16 => {
            let line_start_byte = text.line_to_byte(point.line as usize);
            let line_start_cu = text.byte_to_utf16_cu(line_start_byte);
            let byte = text.utf16_cu_to_byte(line_start_cu + point.character as usize);
            text.byte_to_point(byte)
        }
    }
}

pub fn range(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    range: lsp_types::Range,
) -> PointRange {
    PointRange::new(point(encoding, text, range.start), point(encoding, text, range.end))
}

pub fn deltas(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    edits: impl IntoIterator<Item = lsp_types::TextEdit>,
) -> Deltas<'static> {
    Deltas::new(edits.into_iter().map(|edit| {
        let range = text.point_range_to_byte_range(range(encoding, text, edit.range));
        Delta::new(range, edit.new_text)
    }))
}
