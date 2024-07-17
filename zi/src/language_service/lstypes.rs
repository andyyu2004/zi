//! Types used for the zi language service protocol.
//! All positions are 0-indexed and in UTF-8 code units (bytes).

use std::collections::HashMap;

use url::Url;
pub use zi_core::{CompletionItem, Diagnostic, Point, PointRange};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct InitializeParams {
    pub process_id: u32,
    pub workspace_folders: Vec<lsp_types::WorkspaceFolder>,
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

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct DocumentDiagnosticReport {
    pub diagnostics: Vec<Diagnostic>,
    pub related_documents: HashMap<Url, Vec<Diagnostic>>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Location {
    pub url: Url,
    pub range: PointRange,
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

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct PublishDiagnosticsParams {
    pub url: Url,
    pub diagnostics: Vec<Diagnostic>,
    pub version: Option<i32>,
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
