use std::fmt;

use tokio::sync::watch;

/// A setting that can be read and written to via immutable references.
/// This can be cloned to create multiple references to the same underlying setting.
#[derive(Clone)]
pub struct Setting<T> {
    rx: watch::Receiver<T>,
    tx: watch::Sender<T>,
}

impl<T: PartialEq> PartialEq<T> for Setting<T> {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        &*self.rx.borrow() == other
    }
}

impl<T: fmt::Debug> fmt::Debug for Setting<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Setting({:?})", *self.rx.borrow())
    }
}

impl<T> Setting<T> {
    pub(crate) fn new(value: T) -> Self {
        let (tx, rx) = watch::channel(value);
        Self { rx, tx }
    }

    #[inline]
    pub fn read(&self) -> T
    where
        T: Clone,
    {
        self.rx.borrow().clone()
    }

    #[inline]
    pub fn write(&self, value: T) {
        self.tx.send(value).expect("config receiver should not be dropped");
    }
}
