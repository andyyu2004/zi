use std::fmt;
use std::ops::Deref;

use tokio::sync::watch;

/// A setting that can be read and written to via immutable references.
/// Cloning makes an independent copy holding the current value; use [`Setting::share`] for
/// another handle to the same value.
// TODO find a more generic name as this isn't just used for settings
pub struct Setting<T> {
    rx: watch::Receiver<T>,
    tx: watch::Sender<T>,
}

impl<T: Clone> Clone for Setting<T> {
    fn clone(&self) -> Self {
        Self::new(self.read().clone())
    }
}

impl<T: Default> Default for Setting<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T: PartialEq> PartialEq<T> for Setting<T> {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        &*self.rx.borrow() == other
    }
}

impl<T: PartialEq> PartialEq for Setting<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        *self.read() == *other.read()
    }
}

impl<T: fmt::Debug> fmt::Debug for Setting<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.rx.borrow().deref().fmt(f)
    }
}

impl<T> Setting<T> {
    pub fn new(value: T) -> Self {
        let (tx, rx) = watch::channel(value);
        Self { rx, tx }
    }

    /// Returns another handle to the same value: a write through either is seen by both.
    pub fn share(&self) -> Self {
        Self { rx: self.rx.clone(), tx: self.tx.clone() }
    }

    #[inline]
    pub fn read(&self) -> impl Deref<Target = T> + '_ {
        self.rx.borrow()
    }

    #[inline]
    pub fn write(&self, value: T) {
        self.tx.send(value).expect("config receiver should not be dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::Setting;

    #[test]
    fn clone_is_independent() {
        let a = Setting::new(1);
        let b = a.clone();
        b.write(2);
        assert_eq!(a, 1);
        assert_eq!(b, 2);
    }

    #[test]
    fn share_is_shared() {
        let a = Setting::new(1);
        let b = a.share();
        b.write(2);
        assert_eq!(a, 2);
        a.write(3);
        assert_eq!(b, 3);
    }
}
