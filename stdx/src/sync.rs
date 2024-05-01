use std::sync::atomic::{self, AtomicBool};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct Cancel {
    cancel: Arc<AtomicBool>,
}

impl Cancel {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn cancel(&self) {
        self.cancel.store(true, atomic::Ordering::Relaxed);
    }

    #[inline]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(atomic::Ordering::Relaxed)
    }
}
