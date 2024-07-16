//! Boring impls converting zi to lsp types
//! We always return an `Option` since we don't want to panic if the server is buggy
//! FIXME these should all go to zi-lsp

use zi_core::{CompletionItem, PositionEncoding};
use zi_text::Text;

use crate::Point;

pub fn completions(
    items: impl IntoIterator<Item = lsp_types::CompletionItem>,
) -> impl Iterator<Item = CompletionItem> {
    items.into_iter().map(|item| CompletionItem {
        label: item.label,
        filter_text: item.filter_text,
        insert_text: item.insert_text,
    })
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
