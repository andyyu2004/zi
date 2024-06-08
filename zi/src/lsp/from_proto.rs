//! Boring impls converting zi to lsp types

use zi_core::{Diagnostic, PointRange, Severity};
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

pub fn diagnostics(
    encoding: PositionEncoding,
    text: &(impl Text + ?Sized),
    diagnostics: impl IntoIterator<Item = lsp_types::Diagnostic>,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diag| zi_core::Diagnostic {
            range: range(encoding, text, diag.range),
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
        .collect()
}
