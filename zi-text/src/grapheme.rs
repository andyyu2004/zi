use std::collections::VecDeque;
use std::iter;

use unicode_segmentation::UnicodeSegmentation;
use zi_core::{ByteCol, Point};

use crate::{Text, TextBase as _, TextSlice};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grapheme {
    pub first: char,
    pub len: usize,
}

impl Grapheme {
    fn new(grapheme: &str) -> Self {
        let first = grapheme.chars().next().expect("graphemes are never empty");
        Self { first, len: grapheme.len() }
    }

    fn from_char(c: char) -> Self {
        Self { first: c, len: c.len_utf8() }
    }
}

pub fn graphemes_from<T: Text + ?Sized>(
    text: &T,
    byte: usize,
) -> impl Iterator<Item = Grapheme> + '_ {
    let point = text.byte_to_point(byte);
    let mut line_idx = point.line();
    let mut col = point.byte_col();
    let mut buf = VecDeque::new();
    iter::from_fn(move || {
        loop {
            if let Some(grapheme) = buf.pop_front() {
                return Some(grapheme);
            }

            let line = text.line(line_idx)?;
            let line = line.to_cow();
            buf.extend(line[col..].graphemes(true).map(Grapheme::new));
            buf.extend(terminator(text, line_idx, line.len()).map(Grapheme::from_char));
            line_idx += 1;
            col = 0;
        }
    })
}

pub fn graphemes_before<T: Text + ?Sized>(
    text: &T,
    byte: usize,
) -> impl Iterator<Item = Grapheme> + '_ {
    let point = text.byte_to_point(byte);
    let mut line_idx = Some(point.line());
    let mut col = Some(point.byte_col());
    let mut buf = VecDeque::new();
    iter::from_fn(move || {
        loop {
            if let Some(grapheme) = buf.pop_front() {
                return Some(grapheme);
            }

            let idx = line_idx?;
            let line = text.line(idx);
            let line = line.as_ref().map_or_else(Default::default, TextSlice::to_cow);
            let end = col.take().unwrap_or(line.len());
            buf.extend(line[..end].graphemes(true).rev().map(Grapheme::new));
            line_idx = idx.checked_sub(1);
            if let Some(prev) = line_idx {
                let prev_len = text.line(prev).map_or(0, |line| line.len_bytes());
                buf.extend(terminator(text, prev, prev_len).rev().map(Grapheme::from_char));
            }
        }
    })
}

fn terminator<T: Text + ?Sized>(
    text: &T,
    line_idx: usize,
    line_len: usize,
) -> impl DoubleEndedIterator<Item = char> {
    let start = text.point_to_byte(Point::new(line_idx, 0)) + line_len;
    let end = match text.line(line_idx + 1) {
        Some(_) => text.point_to_byte(Point::new(line_idx + 1, 0)),
        None => text.len_bytes(),
    };
    text.byte_slice(start..end).to_cow().chars().collect::<Vec<_>>().into_iter()
}
