use std::{io, ops};

use grep::matcher::Matcher;
use grep::regex::RegexMatcherBuilder;
use grep::searcher::{BinaryDetection, Searcher, SearcherBuilder, SinkError, SinkMatch};

pub(super) fn matcher(query: &str) -> impl Matcher + Clone {
    let mut builder = RegexMatcherBuilder::new();
    builder.case_smart(true);
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
        .binary_detection(BinaryDetection::quit(b'\x00'))
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

        let byte_range = mat.bytes_range_in_buffer();

        (self.0)(line_number, &matched, byte_range)
    }
}
