use std::fmt;
use std::ops::Deref;

use tokio::sync::watch;

/// A setting that can be read and written to via immutable references.
/// This can be cloned to create multiple references to the same underlying setting.
// TODO find a more generic name as this isn't just used for settings
#[derive(Clone)]
pub struct Setting<T> {
    rx: watch::Receiver<T>,
    tx: watch::Sender<T>,
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

    #[inline]
    pub fn read(&self) -> impl Deref<Target = T> + '_ {
        self.rx.borrow()
    }

    #[inline]
    pub fn write(&self, value: T) {
        self.tx.send(value).expect("config receiver should not be dropped");
    }
}
