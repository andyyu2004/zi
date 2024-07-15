use zi_core::{Point, PointRange, PositionEncoding};
use zi_text::{Deltas, Text};

// For some reason, LSP defines change events that are distinct from `TextEdit`s.
// The former is applied serially, while the latter is applied "atomically".
// However, since our deltas are ordered and disjoint, we can just return them in order and because
// they don't interfere we're all good.
pub fn deltas(
    encoding: PositionEncoding,
    old_text: impl Text,
    deltas: &Deltas<'_>,
) -> Vec<lsp_types::TextDocumentContentChangeEvent> {
    deltas
        .iter()
        .map(|delta| lsp_types::TextDocumentContentChangeEvent {
            range: Some(byte_range(encoding, &old_text, delta.range())),
            text: delta.text().to_string(),
            range_length: None,
        })
        .collect()
}

pub fn byte_range(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    range: std::ops::Range<usize>,
) -> lsp_types::Range {
    lsp_types::Range {
        start: byte(encoding, text, range.start),
        end: byte(encoding, text, range.end),
    }
}

pub fn byte(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    byte: usize,
) -> lsp_types::Position {
    match encoding {
        PositionEncoding::Utf8 => point(encoding, text, text.byte_to_point(byte)),
        PositionEncoding::Utf16 => {
            let line = text.byte_to_line(byte);
            let line_start = text.byte_to_utf16_cu(text.line_to_byte(line));
            let col = text.byte_to_utf16_cu(byte) - line_start;
            lsp_types::Position::new(line as u32, col as u32)
        }
    }
}

pub fn range(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    range: PointRange,
) -> lsp_types::Range {
    lsp_types::Range {
        start: point(encoding, text, range.start()),
        end: point(encoding, text, range.end()),
    }
}

pub fn point(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    point: Point,
) -> lsp_types::Position {
    match encoding {
        PositionEncoding::Utf8 => lsp_types::Position::new(point.line() as u32, point.col() as u32),
        PositionEncoding::Utf16 => byte(encoding, text, text.point_to_byte(point)),
    }
}
