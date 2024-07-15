use url::Url;
use zi_core::EncodedPointRange;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct InitializeParams {
    pub process_id: u32,
    pub capabilities: lsp_types::ClientCapabilities,
    pub workspace_folders: Vec<lsp_types::WorkspaceFolder>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentFormattingParams {
    pub url: Url,
    pub options: lsp_types::FormattingOptions,
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

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Location {
    pub uri: Url,
    pub range: EncodedPointRange,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct TextDocumentPointParams {
    pub url: Url,
    /// The point inside the text document encoded in the service encoding.
    pub point: lsp_types::Position,
}
