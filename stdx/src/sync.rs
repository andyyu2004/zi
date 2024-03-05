use std::sync::atomic::{self, AtomicBool};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Cancel {
    cancel: Arc<AtomicBool>,
}

impl Cancel {
    pub fn new() -> Self {
        Self { cancel: Arc::new(AtomicBool::new(false)) }
    }

    pub fn cancel(&self) {
        self.cancel.store(true, atomic::Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(atomic::Ordering::Relaxed)
    }
}

impl Default for Cancel {
    fn default() -> Self {
        Self::new()
    }
}
