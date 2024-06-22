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

// https://github.com/rust-lang/rust/issues/34433
// https://github.com/rust-lang/rust/pull/66531
pub struct Chain<T, U> {
    chain: core::iter::Chain<T, U>,
    yielded: usize,
    total: usize,
}

pub trait ExactChain<I>: ExactSizeIterator<Item = I> {
    fn exact_chain<U>(self, other: U) -> Chain<Self, U::IntoIter>
    where
        Self: Sized,
        U: IntoIterator<Item = I>,
        U::IntoIter: ExactSizeIterator;
}

impl<I, T> ExactChain<I> for T
where
    T: ExactSizeIterator<Item = I>,
{
    #[inline]
    fn exact_chain<U>(self, other: U) -> Chain<Self, U::IntoIter>
    where
        Self: Sized,
        U: IntoIterator<Item = I>,
        U::IntoIter: ExactSizeIterator,
    {
        let other = other.into_iter();
        Chain { yielded: 0, total: self.len() + other.len(), chain: self.chain(other) }
    }
}

impl<T, I1, I2> Iterator for Chain<I1, I2>
where
    I1: ExactSizeIterator<Item = T>,
    I2: ExactSizeIterator<Item = T>,
{
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.chain.next()?;
        self.yielded += 1;
        Some(item)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact = self.len();
        (exact, Some(exact))
    }
}

impl<T, I1, I2> ExactSizeIterator for Chain<I1, I2>
where
    I1: ExactSizeIterator<Item = T>,
    I2: ExactSizeIterator<Item = T>,
{
    #[inline]
    fn len(&self) -> usize {
        self.total - self.yielded
    }
}
