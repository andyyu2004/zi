use crate::Deltas;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Bias {
    // Important `Left < Right`
    #[default]
    Left,
    Right,
}

slotmap::new_key_type! {
    pub struct MarkId;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MarkEntry {
    // Important we order by byte then bias
    offset: usize,
    bias: Bias,
}

#[derive(Default)]
pub struct MarkTree {
    // A unique sorted list of marks
    marks: Vec<MarkEntry>,
}

impl MarkTree {
    pub fn add(&mut self, byte: usize, bias: Bias) {
        match self.marks.binary_search(&MarkEntry { offset: byte, bias }) {
            Ok(_i) => {}
            Err(i) => {
                let offset = if i > 0 {
                    // TODO: handle bias
                    // Store the offset so we don't have to shift everything all the time
                    debug_assert!(self.marks[i - 1].offset <= byte);
                    byte - self.marks[i - 1].offset
                } else {
                    byte
                };

                // FIXME this is O(n)
                self.marks.insert(i, MarkEntry { offset, bias });
            }
        }
    }

    pub fn edit(&mut self, deltas: &Deltas<'_>) {
        for delta in deltas.iter() {
            let range = delta.range();
            let shift = delta.text().len() as isize - range.len() as isize;
            let i = self
                .marks
                .binary_search_by_key(&(range.start, Bias::Left), |m| (m.offset, m.bias))
                .unwrap_or_else(|i| i);

            if i + 1 < self.marks.len() {
                self.marks[i + 1].offset += shift as usize
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.marks.iter().copied().scan(0, |offset, mark| {
            *offset += mark.offset;
            Some(*offset)
        })
    }
}
