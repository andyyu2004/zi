use std::borrow::Cow;
use std::ops::Range;

use zi_core::{Point, PointRange};
use zi_text::grapheme::graphemes_from;
use zi_text::{PointRangeExt, Text, TextBase, TextSlice};

use super::register::RegisterKind;
use crate::buffer::Buffer;

#[derive(Debug, Clone)]
pub enum Selection {
    Charwise { start: Point, end: Point },
    Line { start_line: usize, end_line: usize },
    Block { lines: Vec<PointRange> },
}

impl Selection {
    pub(crate) fn block(buf: &Buffer, anchor: Point, cursor: Point) -> Self {
        let anchor_span = buf.display_span(anchor);
        let cursor_span = buf.display_span(cursor);
        let cols = anchor_span.start.min(cursor_span.start)..anchor_span.end.max(cursor_span.end);
        let lines = (anchor.line().min(cursor.line())..=anchor.line().max(cursor.line()))
            .map(|line| {
                let bytes = buf.line_byte_range(line, &cols);
                PointRange::new((line, bytes.start), (line, bytes.end))
            })
            .collect();
        Self::Block { lines }
    }

    pub fn content<'a>(&self, text: &'a (impl Text + ?Sized)) -> Cow<'a, str> {
        match self {
            Self::Charwise { start, end } => {
                let start_byte = text.point_to_byte(*start);
                let end_byte = text.point_to_byte(*end) + end_len(text, *end).unwrap_or(0);
                text.byte_slice(start_byte..end_byte).to_cow()
            }
            Self::Line { start_line, end_line } => {
                text.line_slice(*start_line..=*end_line).to_cow()
            }
            Self::Block { lines } => Cow::Owned(
                lines
                    .iter()
                    .map(|range| text.point_slice(*range).to_cow())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        }
    }

    pub fn register_kind(&self) -> RegisterKind {
        match self {
            Self::Charwise { .. } | Self::Block { .. } => RegisterKind::Charwise,
            Self::Line { .. } => RegisterKind::Linewise,
        }
    }

    pub fn byte_ranges(&self, text: &(impl Text + ?Sized)) -> Vec<Range<usize>> {
        match self {
            Self::Charwise { start, end } => {
                let start_byte = text.point_to_byte(*start);
                let end_byte = text.point_to_byte(*end) + end_len(text, *end).unwrap_or(0);
                vec![start_byte..end_byte]
            }
            Self::Line { start_line, end_line } => {
                let start_byte = text.line_to_byte(*start_line);
                let end_byte =
                    text.try_line_to_byte(*end_line + 1).unwrap_or_else(|| text.len_bytes());
                vec![start_byte..end_byte]
            }
            Self::Block { lines } => lines
                .iter()
                .rev()
                .filter(|range| !range.is_empty())
                .map(|range| text.point_range_to_byte_range(*range))
                .collect(),
        }
    }

    pub fn start_point(&self) -> Point {
        match self {
            Self::Charwise { start, .. } => *start,
            Self::Line { start_line, .. } => Point::new(*start_line, 0),
            Self::Block { lines } => lines.first().map_or(Point::new(0, 0), PointRange::start),
        }
    }

    pub fn point_ranges(&self, text: &(impl Text + ?Sized)) -> Vec<PointRange> {
        match self {
            Self::Charwise { start, end } => {
                let end_col = end.byte_col() + end_len(text, *end).unwrap_or(1);
                let range = PointRange::new(*start, Point::new(end.line(), end_col));
                range.explode(text).collect()
            }
            Self::Line { start_line, end_line } => (*start_line..=*end_line)
                .map(|line| {
                    let line_len = text.line(line).map_or(0, |l| l.len_bytes());
                    PointRange::new((line, 0), (line, line_len))
                })
                .collect(),
            Self::Block { lines } => {
                lines.iter().filter(|range| !range.is_empty()).copied().collect()
            }
        }
    }
}

fn end_len(text: &(impl Text + ?Sized), end: Point) -> Option<usize> {
    graphemes_from(text, text.point_to_byte(end)).next().map(|grapheme| grapheme.len)
}
