use std::ops::Range;

use zi_core::{Point, PointRange};
use zi_text::{PointRangeExt, Text, TextBase, TextSlice};

use super::register::RegisterKind;

#[derive(Debug, Clone)]
pub enum VisualSelection {
    Charwise { start: Point, end: Point },
    Line { start_line: usize, end_line: usize },
    Block { start_line: usize, end_line: usize, start_col: usize, end_col: usize },
}

impl VisualSelection {
    pub fn byte_range(&self, text: &(impl Text + ?Sized)) -> Range<usize> {
        match self {
            Self::Charwise { start, end } => {
                let start_byte = text.point_to_byte(*start);
                let end_byte = text.point_to_byte(*end)
                    + text.char_at_point(*end).map_or(0, |c| c.len_utf8());
                start_byte..end_byte
            }
            Self::Line { start_line, end_line } => {
                let start_byte = text.line_to_byte(*start_line);
                let end_byte =
                    text.try_line_to_byte(*end_line + 1).unwrap_or_else(|| text.len_bytes());
                start_byte..end_byte
            }
            Self::Block { .. } => {
                let ranges = self.point_ranges(text);
                if ranges.is_empty() {
                    return 0..0;
                }
                let start_byte = text.point_to_byte(ranges[0].start());
                let last = &ranges[ranges.len() - 1];
                let end_byte = text.point_to_byte(last.end());
                start_byte..end_byte
            }
        }
    }

    pub fn content(&self, text: &(impl Text + ?Sized)) -> String {
        match self {
            Self::Block { start_line, end_line, start_col, end_col } => {
                let mut content = String::new();
                for line_idx in *start_line..=*end_line {
                    if let Some(line) = text.line(line_idx) {
                        let extracted: String =
                            line.chars().skip(*start_col).take(*end_col - *start_col + 1).collect();
                        if !content.is_empty() {
                            content.push('\n');
                        }
                        content.push_str(&extracted);
                    }
                }
                content
            }
            _ => {
                let range = self.byte_range(text);
                text.byte_slice(range).to_cow().into_owned()
            }
        }
    }

    pub fn register_kind(&self) -> RegisterKind {
        match self {
            Self::Charwise { .. } | Self::Block { .. } => RegisterKind::Charwise,
            Self::Line { .. } => RegisterKind::Linewise,
        }
    }

    pub fn point_ranges(&self, text: &(impl Text + ?Sized)) -> Vec<PointRange> {
        match self {
            Self::Charwise { start, end } => {
                let end_col =
                    end.col() + text.char_at_point(*end).map_or(1, |c| c.len_utf8());
                let range = PointRange::new(*start, Point::new(end.line(), end_col));
                range.explode(text).collect()
            }
            Self::Line { start_line, end_line } => {
                let mut ranges = Vec::new();
                for line in *start_line..=*end_line {
                    let line_len = text.line(line).map(|l| l.len_bytes()).unwrap_or(0);
                    ranges.push(PointRange::new(Point::new(line, 0), Point::new(line, line_len)));
                }
                ranges
            }
            Self::Block { start_line, end_line, start_col, end_col } => {
                let max_col = *end_col + 1;
                (*start_line..=*end_line)
                    .map(|line| {
                        PointRange::new(Point::new(line, *start_col), Point::new(line, max_col))
                    })
                    .collect()
            }
        }
    }
}
