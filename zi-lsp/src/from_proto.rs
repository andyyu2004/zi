use async_lsp::lsp_types;
use zi::{lstypes, Delta, Deltas, Diagnostic, Point, PointRange, PositionEncoding, Severity, Text};

pub fn goto_definition(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    res: Option<lsp_types::GotoDefinitionResponse>,
) -> Option<lstypes::GotoDefinitionResponse> {
    let res = match res {
        None => lstypes::GotoDefinitionResponse::Array(vec![]),
        Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) => {
            lstypes::GotoDefinitionResponse::Array(vec![location(encoding, text, loc)?])
        }
        Some(lsp_types::GotoDefinitionResponse::Array(locs)) => {
            lstypes::GotoDefinitionResponse::Array(
                locs.into_iter().filter_map(|loc| location(encoding, text, loc)).collect(),
            )
        }
        Some(lsp_types::GotoDefinitionResponse::Link(links)) => {
            lstypes::GotoDefinitionResponse::Array(
                links
                    .into_iter()
                    .filter_map(|link| {
                        Some(lstypes::Location {
                            url: link.target_uri,
                            range: range(encoding, text, link.target_selection_range)?,
                        })
                    })
                    .collect(),
            )
        }
    };
    Some(res)
}

pub fn location(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    loc: lsp_types::Location,
) -> Option<lstypes::Location> {
    Some(lstypes::Location { url: loc.uri, range: range(encoding, text, loc.range)? })
}

pub fn deltas(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    edits: impl IntoIterator<Item = lsp_types::TextEdit, IntoIter: ExactSizeIterator>,
) -> Option<Deltas<'static>> {
    let edits = edits.into_iter();
    let n = edits.len();
    let deltas = Deltas::new(edits.filter_map(|edit| {
        let range = text.point_range_to_byte_range(range(encoding, text, edit.range)?);
        Some(Delta::new(range, edit.new_text))
    }));

    // If any of the edits were invalid, return None.
    if deltas.len() < n { None } else { Some(deltas) }
}

pub fn range(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    range: lsp_types::Range,
) -> Option<PointRange> {
    Some(PointRange::new(point(encoding, text, range.start)?, point(encoding, text, range.end)?))
}

pub fn point(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    point: lsp_types::Position,
) -> Option<Point> {
    if point.line as usize > text.len_lines() {
        return None;
    }

    match encoding {
        PositionEncoding::Utf8 => Some(Point::new(point.line as usize, point.character as usize)),
        PositionEncoding::Utf16 => {
            let line_start_byte = text.line_to_byte(point.line as usize);
            let line_start_cu = text.byte_to_utf16_cu(line_start_byte);
            if line_start_cu + point.character as usize > text.len_utf16_cu() {
                return None;
            }

            let byte = text.utf16_cu_to_byte(line_start_cu + point.character as usize);
            Some(text.byte_to_point(byte))
        }
    }
}

pub fn diagnostics(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    diags: impl IntoIterator<Item = lsp_types::Diagnostic>,
) -> Vec<Diagnostic> {
    diags.into_iter().filter_map(|diag| diagnostic(encoding, text, diag)).collect()
}

pub fn diagnostic(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    diag: lsp_types::Diagnostic,
) -> Option<Diagnostic> {
    Some(Diagnostic {
        range: range(encoding, &text, diag.range)?,
        severity: match diag.severity {
            Some(lsp_types::DiagnosticSeverity::ERROR) => Severity::Error,
            Some(lsp_types::DiagnosticSeverity::WARNING) => Severity::Warning,
            Some(lsp_types::DiagnosticSeverity::INFORMATION) => Severity::Info,
            Some(lsp_types::DiagnosticSeverity::HINT) => Severity::Hint,
            // Assume error if unspecified
            _ => Severity::Error,
        },
        message: diag.message,
    })
}

pub fn completion_response(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    res: lsp_types::CompletionResponse,
) -> lstypes::CompletionResponse {
    let items = match res {
        lsp_types::CompletionResponse::Array(items) => items,
        lsp_types::CompletionResponse::List(list) => list.items,
    };

    lstypes::CompletionResponse {
        items: items.into_iter().filter_map(|item| completion_item(encoding, text, item)).collect(),
    }
}

pub fn completion_item(
    _encoding: PositionEncoding,
    _text: &(impl Text + ?Sized),
    item: lsp_types::CompletionItem,
) -> Option<lstypes::CompletionItem> {
    Some(lstypes::CompletionItem {
        label: item.label,
        insert_text: item.insert_text,
        filter_text: item.filter_text,
    })
}
