/// An iterator that can move in both directions.
/// Not to be confused with the `DoubleEndedIterator` trait, which is for iterators that can be iterated from the back.
pub trait BidirectionalIterator: Iterator {
    fn prev(&mut self) -> Option<Self::Item>;

    /// Returns a new iterator that iterates in the opposite direction.
    fn reversed(self) -> Reversed<Self>
    where
        Self: Sized,
    {
        Reversed { iter: self }
    }
}

impl<I: BidirectionalIterator + ?Sized> BidirectionalIterator for Box<I> {
    #[inline]
    fn prev(&mut self) -> Option<Self::Item> {
        (**self).prev()
    }
}

impl<I: BidirectionalIterator + ?Sized> BidirectionalIterator for &mut I {
    #[inline]
    fn prev(&mut self) -> Option<Self::Item> {
        (**self).prev()
    }
}

pub struct Reversed<I> {
    iter: I,
}

impl<I: BidirectionalIterator> Iterator for Reversed<I> {
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.prev()
    }
}

impl<I: BidirectionalIterator> BidirectionalIterator for Reversed<I> {
    #[inline]
    fn prev(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

#[cfg(feature = "ropey")]
mod ropey {
    use super::BidirectionalIterator;

    impl BidirectionalIterator for ropey::iter::Chars<'_> {
        #[inline]
        fn prev(&mut self) -> Option<Self::Item> {
            self.prev()
        }
    }
}
