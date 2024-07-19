//! Types used for the zi language service protocol.
//! All positions are 0-indexed and in UTF-8 code units (bytes).

use std::collections::HashMap;
use std::fmt;

use url::Url;
pub use zi_core::{CompletionItem, Point, PointRange};
use zi_text::Text;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct InitializeParams {
    pub process_id: u32,
    pub workspace_folders: Vec<WorkspaceFolder>,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
pub struct WorkspaceFolder {
    pub uri: Url,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentFormattingParams {
    pub url: Url,
    pub options: FormattingOptions,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FormattingOptions {
    pub tab_size: u32,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct GotoDefinitionParams {
    pub at: TextDocumentPointParams,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct ReferenceParams {
    pub at: TextDocumentPointParams,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum GotoDefinitionResponse {
    Array(Vec<Location>),
}

impl Default for GotoDefinitionResponse {
    #[inline]
    fn default() -> Self {
        GotoDefinitionResponse::Array(Default::default())
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct DocumentDiagnosticReport {
    pub diagnostics: Diagnostics,
    pub related_documents: HashMap<Url, Diagnostics>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Diagnostics {
    Full(Vec<Diagnostic>),
    Unchanged,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Location {
    pub url: Url,
    pub range: EncodedRange,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct EncodedRange {
    /// Special case, this range is in `encoding` units, not necessarily `Utf8.
    range: PointRange,
    encoding: PositionEncoding,
}

impl fmt::Display for EncodedRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.range)
    }
}

impl EncodedRange {
    pub fn new(encoding: PositionEncoding, range: PointRange) -> Self {
        Self { encoding, range }
    }

    pub fn start(&self) -> EncodedPoint {
        EncodedPoint { point: self.range.start(), encoding: self.encoding }
    }

    pub fn end(&self) -> EncodedPoint {
        EncodedPoint { point: self.range.end(), encoding: self.encoding }
    }

    pub fn decode(&self, text: impl Text) -> Option<PointRange> {
        text.decode_range(self)
    }

    pub fn decode_start(&self, text: impl Text) -> Option<Point> {
        text.decode_point(self.start())
    }
}

#[derive(Debug, Eq, PartialEq, Clone, PartialOrd, Ord)]
pub struct EncodedPoint {
    point: Point,
    encoding: PositionEncoding,
}

impl From<Point> for EncodedPoint {
    #[inline]
    fn from(point: Point) -> Self {
        Self { point, encoding: PositionEncoding::Utf8 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PositionEncoding {
    /// UTF-8 code units (bytes) (not codepoints I think, but can't find conclusive documentation?)
    Utf8,
    /// UTF-16 code units
    #[default]
    Utf16,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct TextDocumentPointParams {
    pub url: Url,
    pub point: Point,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct DocumentDiagnosticParams {
    pub url: Url,
    // pub previous_result_id: Option<String>,
}

// #[derive(Debug, Eq, PartialEq, Clone)]
// pub struct PublishDiagnosticsParams {
//     pub url: Url,
//     pub diagnostics: Vec<Diagnostic>,
//     pub version: Option<i32>,
// }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: EncodedRange,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum Severity {
    Hint,
    Info,
    Warning,
    Error,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct CompletionParams {
    pub at: TextDocumentPointParams,
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct CompletionResponse {
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct SemanticTokensParams {
    pub url: Url,
}

pub(crate) trait TextExt {
    fn decode_point(&self, point: EncodedPoint) -> Option<Point>;

    #[inline]
    fn decode_range(&self, range: &EncodedRange) -> Option<PointRange> {
        let start = self.decode_point(range.start())?;
        let end = self.decode_point(range.end())?;
        Some(PointRange::new(start, end))
    }
}

impl<T: Text> TextExt for T {
    fn decode_point(&self, EncodedPoint { point, encoding }: EncodedPoint) -> Option<Point> {
        if point.line() as usize > self.len_lines() {
            return None;
        }

        match encoding {
            PositionEncoding::Utf8 => Some(Point::new(point.line(), point.col())),
            PositionEncoding::Utf16 => {
                let line_start_byte = self.line_to_byte(point.line() as usize);
                let line_start_cu = self.byte_to_utf16_cu(line_start_byte);
                if line_start_cu + point.col() as usize > self.len_utf16_cu() {
                    return None;
                }

                let byte = self.utf16_cu_to_byte(line_start_cu + point.col() as usize);
                Some(self.byte_to_point(byte))
            }
        }
    }
}
