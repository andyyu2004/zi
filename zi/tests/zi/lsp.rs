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

macro_rules! lsp_pos {
    ($line:literal:$character:literal) => {
        lsp_types::Position { line: $line, character: $character }
    };
}

macro_rules! lsp_range {
    ($start_line:literal:$start_character:literal..$end_line:literal:$end_character:literal) => {
        lsp_types::Range {
            start: lsp_pos!($start_line:$start_character),
            end: lsp_pos!($end_line:$end_character),
        }
    };
}

macro_rules! lsp_change_event {
    ($start_line:literal:$start_character:literal..$end_line:literal:$end_character:literal =>$text:expr) => {
        lsp_types::TextDocumentContentChangeEvent {
            range: Some(lsp_range!($start_line:$start_character..$end_line:$end_character)),
            text: $text.to_string(),
            range_length: None,
        }
    };
    ($text:expr) => {
        lsp_types::TextDocumentContentChangeEvent {
            range: None,
            text: $text.to_string(),
            range_length: None,
        }
    };
}

use {lsp_change_event, lsp_pos, lsp_range};
