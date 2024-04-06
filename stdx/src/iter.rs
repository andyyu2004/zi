use std::iter::{FusedIterator, Peekable};

pub trait IteratorExt: Iterator {
    /// Returns an iterator that yields the elements of the underlying iterator,
    /// or a single item of a default value if the underlying iterator is empty.
    #[inline]
    fn default_if_empty(self, default: Self::Item) -> DefaultIfEmpty<Self>
    where
        Self: Sized,
    {
        DefaultIfEmpty::new(self, default)
    }
}

impl<I: Iterator> IteratorExt for I {}

pub struct DefaultIfEmpty<I: Iterator> {
    iter: Peekable<I>,
    default: Option<I::Item>,
    empty: bool,
}

impl<I: Iterator> DefaultIfEmpty<I> {
    #[inline]
    fn new(iter: I, default: I::Item) -> Self {
        let mut iter = iter.peekable();
        let empty = iter.peek().is_none();
        DefaultIfEmpty { iter, empty, default: Some(default) }
    }
}

impl<I: Iterator> Iterator for DefaultIfEmpty<I> {
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.empty { self.default.take() } else { self.iter.next() }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.empty { (1, Some(1)) } else { self.iter.size_hint() }
    }
}

impl<I: DoubleEndedIterator> DoubleEndedIterator for DefaultIfEmpty<I> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.empty { self.default.take() } else { self.iter.next_back() }
    }
}

impl<I: ExactSizeIterator> ExactSizeIterator for DefaultIfEmpty<I> {}

impl<I: FusedIterator> FusedIterator for DefaultIfEmpty<I> {}
