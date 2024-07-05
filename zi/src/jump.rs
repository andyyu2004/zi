use std::collections::VecDeque;
use std::fmt;

#[derive(Clone)]
pub struct JumpList<T> {
    jumps: VecDeque<T>,
    current: usize,
    capacity: usize,
}

impl<T: fmt::Debug> fmt::Debug for JumpList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, jump) in self.jumps.iter().enumerate() {
            write!(f, "{jump:?}")?;
            if i == self.current {
                write!(f, " <<<")?;
            }

            if i < self.jumps.len() - 1 {
                writeln!(f)?;
            }
        }

        if self.current == self.jumps.len() {
            write!(f, "\n <<<")?;
        }

        Ok(())
    }
}

impl<T> Default for JumpList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> JumpList<T> {
    pub fn new() -> Self {
        Self::with_capacity(100)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        // not strictly necessary probably, but the edge cases are not tested
        assert!(capacity >= 3, "capacity must be greater than 2");
        // Don't need to allocate the full capacity upfront
        Self { jumps: Default::default(), current: 0, capacity }
    }
}

impl<T: PartialEq> JumpList<T> {
    /// Push a value onto the jump list. If the value is the same as the last value, it will not be pushed.
    /// NOTE: To make this work as intended you should push the current location, not the new location.
    pub fn push(&mut self, val: T) {
        // avoid pushing the same value twice in a row
        if Some(&val) == self.jumps.back() {
            return;
        }

        if self.current == self.capacity {
            self.jumps.pop_front();
        }

        self.jumps.truncate(1 + self.current);
        self.jumps.push_back(val);
        self.jumps.shrink_to(self.capacity);
        self.current = self.jumps.len();

        debug_assert!(self.invariants());
    }

    fn invariants(&self) -> bool {
        self.jumps.len() <= self.capacity && self.current <= self.jumps.len()
    }

    pub fn next(&mut self) -> Option<&T> {
        if self.current + 1 >= self.jumps.len() {
            return None;
        }

        self.current += 1;
        let v = self.jumps.get(self.current);
        debug_assert!(self.invariants());
        v
    }

    /// Go back to the previous jump location.
    /// We require that the caller provides the current location, so it's possible to go forward again.
    pub fn prev(&mut self, from: T) -> Option<&T> {
        let mut current = self.current.checked_sub(1)?;
        if self.current == self.jumps.len() {
            if self.jumps.len() == self.capacity {
                // If we're already full, the push is going to drop an element so we adjust the index accordingly
                current = current
                    .checked_sub(1)
                    .expect("we can't be full and empty because we've checked capacity > 0")
            }
            self.push(from);
        }

        self.current = current;

        self.jumps.get(current)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.jumps.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_list_edge_case() {
        let mut list = JumpList::with_capacity(100);
        list.push(1);
        assert_eq!(list.prev(2), Some(&1));
        assert_eq!(list.next(), Some(&2));
        assert_eq!(list.next(), None);
        // confusingly, 1 is the correct result (not 2) because otherwise we'd be jumping to the
        // same spot as the previous non-none result.
        assert_eq!(list.prev(3), Some(&1));
    }

    #[test]
    fn jump_list_smoke() {
        let mut list = JumpList::with_capacity(3);
        assert!(list.next().is_none());
        assert!(list.next().is_none());
        assert!(list.prev(42).is_none());
        assert!(list.prev(42).is_none());

        list.push(1);
        list.push(2);
        list.push(3);
        list.push(4);
        list.push(4);

        assert_eq!(list.jumps, [2, 3, 4]);

        assert_eq!(list.next(), None);
        assert_eq!(list.next(), None);
        assert_eq!(list.next(), None);

        assert_eq!(list.prev(42), Some(&4));
        assert_eq!(list.next(), Some(&42));
        assert_eq!(list.next(), None);
        assert_eq!(list.next(), None);
        assert_eq!(list.prev(42), Some(&4));
        assert_eq!(list.prev(42), Some(&3));

        assert_eq!(list.jumps, [3, 4, 42]);

        list.push(5);
        assert_eq!(list.jumps, [3, 5]);
        assert_eq!(list.next(), None);
        assert_eq!(list.prev(42), Some(&5));
        assert_eq!(list.prev(42), Some(&3));

        list.push(6);
        assert_eq!(list.jumps, [3, 6]);
        list.push(7);
        assert_eq!(list.jumps, [3, 6, 7]);
        list.push(8);
        assert_eq!(list.jumps, [6, 7, 8]);
    }
}
