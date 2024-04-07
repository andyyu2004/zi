use std::iter::{FusedIterator, Peekable};

pub trait IteratorExt: Iterator {
    /// Returns an iterator that yields the elements of the underlying iterator,
    /// or a single item of a default value if the underlying iterator is empty.
    #[inline]
    fn default_if_empty<F>(self, default: F) -> DefaultIfEmpty<Self, F>
    where
        Self: Sized,
    {
        DefaultIfEmpty::new(self, default)
    }
}

impl<I: Iterator> IteratorExt for I {}

pub struct DefaultIfEmpty<I: Iterator, F> {
    iter: Peekable<I>,
    default: Option<F>,
    empty: bool,
}

impl<I: Iterator, F> DefaultIfEmpty<I, F> {
    #[inline]
    fn new(iter: I, default: F) -> Self {
        let mut iter = iter.peekable();
        let empty = iter.peek().is_none();
        DefaultIfEmpty { iter, empty, default: Some(default) }
    }
}

impl<I, F> Iterator for DefaultIfEmpty<I, F>
where
    I: Iterator,
    F: FnOnce() -> I::Item,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.empty { Some(self.default.take()?()) } else { self.iter.next() }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.empty { (1, Some(1)) } else { self.iter.size_hint() }
    }
}

impl<I, F> DoubleEndedIterator for DefaultIfEmpty<I, F>
where
    I: DoubleEndedIterator,
    F: FnOnce() -> I::Item,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.empty { Some(self.default.take()?()) } else { self.iter.next_back() }
    }
}

impl<I, F> ExactSizeIterator for DefaultIfEmpty<I, F>
where
    I: ExactSizeIterator,
    F: FnOnce() -> I::Item,
{
}

impl<I, F> FusedIterator for DefaultIfEmpty<I, F>
where
    I: FusedIterator,
    F: FnOnce() -> I::Item,
{
}
