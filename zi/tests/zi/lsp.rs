use super::*;

mod change;
mod encoding;
mod fake;
mod format;

use std::fmt;
use std::sync::atomic::{self, AtomicUsize};

pub use fake::{FakeLanguageServer, FakeLanguageServerBuilder};
use lsp_types::{notification, request, OneOf};

// Utility type that can be referenced from within `Fn` closures.
pub struct ExpectedSequence<T> {
    xs: Vec<T>,
    idx: AtomicUsize,
}

impl<T> ExpectedSequence<T> {
    pub fn new(xs: impl Into<Vec<T>>) -> Self {
        Self { xs: xs.into(), idx: AtomicUsize::new(0) }
    }

    #[track_caller]
    pub fn assert_eq(&self, expected: &T)
    where
        T: fmt::Debug + PartialEq,
    {
        let idx = self.idx.fetch_add(1, atomic::Ordering::Relaxed);
        let actual = self.xs.get(idx).unwrap_or_else(|| {
            panic!("ExpectedSequence: out of bounds access at index {idx}, expected {expected:?}")
        });

        assert_eq!(actual, expected, "ExpectedSequence: mismatch at index {idx}");
    }
}
