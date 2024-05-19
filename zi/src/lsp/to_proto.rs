use zi_core::{Point, PointRange};
use zi_lsp::{lsp_types, OffsetEncoding};
use zi_text::{Deltas, Text};

// For some reason, LSP defines change events that are distinct from `TextEdit`s.
// The former is applied serially, while the latter is applied "atomically".
// Not sure why this distinction exists, but it's annoying.
pub fn deltas_to_events(
    encoding: OffsetEncoding,
    old_text: impl Text,
    deltas: &Deltas<'_>,
) -> Vec<lsp_types::TextDocumentContentChangeEvent> {
    deltas
        .iter()
        .map(|delta| {
            let r = old_text.byte_range_to_point_range(&delta.range());
            lsp_types::TextDocumentContentChangeEvent {
                range: Some(range(encoding, &old_text, r)),
                text: delta.text().to_string(),
                range_length: None,
            }
        })
        .collect()
}

pub fn range(encoding: OffsetEncoding, text: impl Text, range: PointRange) -> lsp_types::Range {
    lsp_types::Range {
        start: point(encoding, &text, range.start()),
        end: point(encoding, &text, range.end()),
    }
}

pub fn point(encoding: OffsetEncoding, text: impl Text, point: Point) -> lsp_types::Position {
    // TODO this needs to consider offset encoding
    lsp_types::Position { line: point.line() as u32, character: point.col() as u32 }
}
