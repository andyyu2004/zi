use std::{io, ops};

use grep::matcher::Matcher;
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder, SinkError, SinkMatch};

use crate::BufferId;

const NUL: u8 = 0;

#[derive(Default)]
pub(super) struct SearchState {
    pub(super) last_update: (BufferId, String),
    /// Whether to highlight search matches
    pub(super) hlsearch: bool,
    matches: Vec<Match>,
    match_idx: usize,
}

impl SearchState {
    /// Prepare to update the search state for a new query, if no change is needed, return false.
    /// Otherwise, return true and update the state. The caller must then call `set_matches`.
    pub(super) fn prepare_update(&mut self, buffer_id: BufferId, query: &str) -> bool {
        let (last_buffer_id, last_query) = &self.last_update;
        if buffer_id != *last_buffer_id || query != *last_query {
            self.last_update = (buffer_id, query.to_string());
            return true;
        }
        false
    }

    pub(super) fn matches(&self) -> &[Match] {
        &self.matches
    }

    pub(super) fn set_matches(&mut self, start_byte: usize, matches: impl Into<Vec<Match>>) {
        self.matches = matches.into();
        self.match_idx =
            self.matches.iter().position(|m| m.byte_range.start >= start_byte).unwrap_or(0);
    }

    pub(super) fn current_match_idx(&self) -> usize {
        self.match_idx
    }

    pub(super) fn current_match(&self) -> Option<&Match> {
        self.matches.get(self.match_idx)
    }

    pub(super) fn next_match(&mut self) -> Option<&Match> {
        if self.matches.is_empty() {
            return None;
        }

        self.match_idx = (self.match_idx + 1) % self.matches.len();
        self.matches.get(self.match_idx)
    }

    pub(super) fn prev_match(&mut self) -> Option<&Match> {
        if self.match_idx == 0 {
            self.match_idx = self.matches.len() - 1;
        } else {
            self.match_idx -= 1;
        }

        self.matches.get(self.match_idx)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub(crate) byte_range: ops::Range<usize>,
}

impl Match {
    #[inline]
    pub fn range(&self) -> ops::Range<usize> {
        self.byte_range.clone()
    }
}

pub(super) fn matcher(query: &str) -> impl Matcher + Clone {
    let mut builder = RegexMatcherBuilder::new();
    builder.case_smart(true).ban_byte(Some(NUL)).whole_line(false);
    match builder.build(query) {
        Ok(matcher) => matcher,
        // if the regex is invalid, just treat the query as literal instead of a regex
        Err(_err) => builder
            .fixed_strings(true)
            .build(query)
            .expect("don't think it can fail with fixed strings"),
    }
}

pub(super) fn searcher() -> Searcher {
    SearcherBuilder::new()
        // maybe there's stronger heuristic, but a null byte is probably a decent indicator
        .binary_detection(BinaryDetection::quit(NUL))
        .line_number(true)
        .build()
}

#[derive(Clone, Debug)]
pub struct Sink<F>(pub F)
where
    F: FnMut(u64, &str, ops::Range<usize>) -> io::Result<bool>;

impl<F> grep::searcher::Sink for Sink<F>
where
    F: FnMut(u64, &str, ops::Range<usize>) -> Result<bool, io::Error>,
{
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, io::Error> {
        let matched = match std::str::from_utf8(mat.bytes()) {
            Ok(matched) => matched,
            Err(err) => return Err(io::Error::error_message(err)),
        };

        let line_number = match mat.line_number() {
            Some(line_number) => line_number,
            None => {
                let msg = "line numbers not enabled";
                return Err(io::Error::error_message(msg));
            }
        };

        let start_byte = mat.absolute_byte_offset() as usize;
        let mut byte_range = start_byte..start_byte + matched.len();
        if matched.ends_with('\n') {
            byte_range.end -= 1;
        }

        (self.0)(line_number, matched, byte_range)
    }
}
