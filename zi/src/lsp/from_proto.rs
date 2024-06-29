//! Boring impls converting zi to lsp types
//! We always return an `Option` since we don't want to panic if the server is buggy

use zi_core::{Diagnostic, PointRange, Severity};
use zi_lsp::{lsp_types, PositionEncoding};
use zi_text::{Delta, Deltas, Text};

use crate::Point;

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

pub fn range(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    range: lsp_types::Range,
) -> Option<PointRange> {
    Some(PointRange::new(point(encoding, text, range.start)?, point(encoding, text, range.end)?))
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

pub fn diagnostic(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    diag: lsp_types::Diagnostic,
) -> Option<Diagnostic> {
    Some(zi_core::Diagnostic {
        range: range(encoding, text, diag.range)?,
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
