// Just an undo stack for now
#[derive(Debug)]
pub(crate) struct UndoTree<T> {
    // TODO depth limit
    revisions: Vec<T>,
    current: usize,
}

impl<T> UndoTree<T> {
    pub fn push(&mut self, item: T) {
        self.revisions.truncate(self.current);
        self.revisions.push(item);
        self.current += 1;
    }

    pub fn undo(&mut self) -> Option<&T> {
        self.current = self.current.checked_sub(1)?;
        self.revisions.get(self.current)
    }

    pub fn redo(&mut self) -> Option<&T> {
        if self.current < self.revisions.len() {
            let rev = self.revisions.get(self.current);
            self.current += 1;
            rev
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.revisions.clear();
        self.current = 0;
    }
}

impl<T> Default for UndoTree<T> {
    fn default() -> Self {
        Self { revisions: Default::default(), current: 0 }
    }
}

#[cfg(test)]
mod tests;
