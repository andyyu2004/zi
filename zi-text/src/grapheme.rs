use unicode_segmentation::UnicodeSegmentation;
use zi_core::ByteCol;

pub fn floor_grapheme_boundary(line: &str, byte_col: ByteCol) -> ByteCol {
    if byte_col >= line.len() {
        return line.len();
    }
    line.grapheme_indices(true)
        .map(|(start, _)| start)
        .take_while(|&start| start <= byte_col)
        .last()
        .unwrap_or(0)
}

pub fn next_grapheme_boundary(line: &str, byte_col: ByteCol, n: usize) -> ByteCol {
    let col = floor_grapheme_boundary(line, byte_col);
    line[col..].grapheme_indices(true).nth(n).map_or(line.len(), |(start, _)| col + start)
}

pub fn prev_grapheme_boundary(line: &str, byte_col: ByteCol, n: usize) -> ByteCol {
    let col = floor_grapheme_boundary(line, byte_col);
    line[..col].grapheme_indices(true).rev().take(n).last().map_or(col, |(start, _)| start)
}
