use async_lsp::lsp_types;
use zi::{lstypes, Deltas, Point, Text};

pub fn goto_definition(
    encoding: lstypes::PositionEncoding,
    text: &(impl Text + ?Sized),
    params: lstypes::GotoDefinitionParams,
) -> lsp_types::GotoDefinitionParams {
    lsp_types::GotoDefinitionParams {
        text_document_position_params: document_position(encoding, text, params.at),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    }
}

// For some reason, LSP defines change events that are distinct from `TextEdit`s.
// The former is applied serially, while the latter is applied "atomically".
// However, since our deltas are ordered and disjoint, we can just return them in order and because
// they don't interfere we're all good.
pub fn deltas(
    encoding: lstypes::PositionEncoding,
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
    encoding: lstypes::PositionEncoding,
    text: &(impl Text + ?Sized),
    range: std::ops::Range<usize>,
) -> lsp_types::Range {
    lsp_types::Range {
        start: byte(encoding, text, range.start),
        end: byte(encoding, text, range.end),
    }
}

pub fn byte(
    encoding: lstypes::PositionEncoding,
    text: &(impl Text + ?Sized),
    byte: usize,
) -> lsp_types::Position {
    match encoding {
        lstypes::PositionEncoding::Utf8 => point(encoding, text, text.byte_to_point(byte)),
        lstypes::PositionEncoding::Utf16 => {
            let line = text.byte_to_line(byte);
            let line_start = text.byte_to_utf16_cu(text.line_to_byte(line));
            let col = text.byte_to_utf16_cu(byte) - line_start;
            lsp_types::Position::new(line as u32, col as u32)
        }
    }
}

pub fn point(
    encoding: lstypes::PositionEncoding,
    text: &(impl Text + ?Sized),
    point: Point,
) -> lsp_types::Position {
    match encoding {
        lstypes::PositionEncoding::Utf8 => {
            lsp_types::Position::new(point.line() as u32, point.col() as u32)
        }
        lstypes::PositionEncoding::Utf16 => byte(encoding, text, text.point_to_byte(point)),
    }
}

pub fn document_position(
    encoding: lstypes::PositionEncoding,
    text: &(impl Text + ?Sized),
    params: lstypes::TextDocumentPointParams,
) -> lsp_types::TextDocumentPositionParams {
    lsp_types::TextDocumentPositionParams {
        text_document: lsp_types::TextDocumentIdentifier { uri: params.url },
        position: point(encoding, &text, params.point),
    }
}
