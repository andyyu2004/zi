use std::borrow::Cow;
use std::fmt;
use std::sync::atomic::{self, AtomicUsize};
use std::sync::Arc;

/// A clone and thread-safe implementation of `DropBomb`.
#[derive(Clone)]
pub struct DropBomb {
    inner: Arc<Inner>,
}

impl fmt::Debug for DropBomb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DropBomb")
            .field("msg", &self.inner.msg)
            .field("count", &self.inner.count.load(atomic::Ordering::Acquire))
            .finish()
    }
}

impl DropBomb {
    pub fn new(msg: &'static str) -> Self {
        Self { inner: Arc::new(Inner { msg: Cow::Borrowed(msg), count: AtomicUsize::new(1) }) }
    }

    /// Disarm the bomb, preventing it from triggering when dropped.
    pub fn defuse(&self) {
        self.inner.count.store(0, atomic::Ordering::Release);
    }
}

impl Drop for DropBomb {
    fn drop(&mut self) {
        if self.inner.count.load(atomic::Ordering::Acquire) > 0 && !std::thread::panicking() {
            panic!("{}", self.inner.msg)
        }
    }
}

struct Inner {
    msg: Cow<'static, str>,
    // Using a usize instead of bool here since this can be generated as a counter
    count: AtomicUsize,
}
