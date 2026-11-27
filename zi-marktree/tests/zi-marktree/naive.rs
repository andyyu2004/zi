use std::collections::BTreeMap;
use std::ops::Range;

use zi_marktree::Bias;

use crate::Id;

/// The obvious implementation of `MarkTree`'s API, to test it against: every mark's range and
/// biases in a map, with `shift` moving each mark.
#[derive(Debug)]
pub struct NaiveMarks {
    len: usize,
    marks: BTreeMap<Id, Mark>,
}

#[derive(Debug)]
struct Mark {
    range: Range<usize>,
    start_bias: Bias,
    end_bias: Bias,
}

impl NaiveMarks {
    pub fn new(len: usize) -> Self {
        Self { len, marks: BTreeMap::new() }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn insert(&mut self, at: usize, width: usize, id: Id, start_bias: Bias, end_bias: Bias) {
        assert!(at + width <= self.len, "{at}..{} out of bounds of {}", at + width, self.len);
        // A point mark has a single key, so both ends move with the start bias.
        let end_bias = if width == 0 { start_bias } else { end_bias };
        self.marks.insert(id, Mark { range: at..at + width, start_bias, end_bias });
    }

    pub fn get(&self, id: Id) -> Option<Range<usize>> {
        self.marks.get(&id).map(|mark| mark.range.clone())
    }

    pub fn delete(&mut self, id: Id) -> Option<Range<usize>> {
        self.marks.remove(&id).map(|mark| mark.range)
    }

    /// Replaces the bytes in `range` with `by` bytes.
    pub fn shift(&mut self, range: Range<usize>, by: usize) {
        assert!(range.end <= self.len, "{range:?} out of bounds of {}", self.len);
        self.len = self.len - range.len() + by;
        for mark in self.marks.values_mut() {
            let Range { start, end } = mark.range;
            mark.range = shift_offset(start, mark.start_bias, &range, by)
                ..shift_offset(end, mark.end_bias, &range, by);
        }
    }

    /// The marks whose start is in `range`, sorted by start, then id.
    pub fn range(&self, range: Range<usize>) -> Vec<(Range<usize>, Id)> {
        let mut items = self
            .marks
            .iter()
            .filter(|(_, mark)| range.contains(&mark.range.start))
            .map(|(&id, mark)| (mark.range.clone(), id))
            .collect::<Vec<_>>();
        items.sort_by_key(|&(ref range, id)| (range.start, id));
        items
    }

    /// Every mark, sorted by start, then id.
    pub fn all(&self) -> Vec<(Range<usize>, Id)> {
        self.range(0..self.len + 1)
    }

    /// Removes the marks `range` returns and returns them.
    pub fn drain(&mut self, range: Range<usize>) -> Vec<(Range<usize>, Id)> {
        let items = self.range(range);
        for &(_, id) in &items {
            self.marks.remove(&id);
        }
        items
    }

    pub fn ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.marks.keys().copied()
    }
}

/// Where a key at `at` with the given bias is after `shift(range, by)`.
fn shift_offset(at: usize, bias: Bias, range: &Range<usize>, by: usize) -> usize {
    if at < range.start || (at == range.start && bias == Bias::Left) {
        at
    } else if at <= range.end {
        range.start + by
    } else {
        at - range.len() + by
    }
}
