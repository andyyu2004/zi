use crate::Deltas;

pub struct MarkTree {
    marks: Vec<usize>,
}

impl MarkTree {
    pub fn add(&mut self, byte: usize) {}

    pub fn edit(&mut self, deltas: &Deltas<'_>) {}

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.marks.iter().copied()
    }
}
