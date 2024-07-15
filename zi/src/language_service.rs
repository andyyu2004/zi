use futures_core::future::BoxFuture;
use zi_core::PositionEncoding;
use zi_lsp::lsp_types;

use crate::Result;
// TODO using lsp_types for now, but should define our own interface to drop the dependency;

/// An abstraction of language server requests. Notifications are handled via the event system.
pub trait LanguageService: Send {
    fn initialize(
        &mut self,
        params: lsp_types::InitializeParams,
    ) -> ResponseFuture<lsp_types::InitializeResult>;

    fn formatting(
        &mut self,
        params: lsp_types::DocumentFormattingParams,
    ) -> ResponseFuture<Option<Vec<lsp_types::TextEdit>>>;

    fn definition(
        &mut self,
        params: lsp_types::GotoDefinitionParams,
    ) -> ResponseFuture<Option<lsp_types::GotoDefinitionResponse>>;

    fn references(
        &mut self,
        params: lsp_types::ReferenceParams,
    ) -> ResponseFuture<Option<Vec<lsp_types::Location>>>;

    fn type_definition(
        &mut self,
        params: lsp_types::GotoDefinitionParams,
    ) -> ResponseFuture<Option<lsp_types::GotoDefinitionResponse>>;

    fn implementation(
        &mut self,
        params: lsp_types::GotoDefinitionParams,
    ) -> ResponseFuture<Option<lsp_types::GotoDefinitionResponse>>;

    fn completion(
        &mut self,
        params: lsp_types::CompletionParams,
    ) -> ResponseFuture<Option<lsp_types::CompletionResponse>>;

    fn semantic_tokens_full(
        &mut self,
        params: lsp_types::SemanticTokensParams,
    ) -> ResponseFuture<Option<lsp_types::SemanticTokensResult>>;

    fn document_diagnostic(
        &mut self,
        params: lsp_types::DocumentDiagnosticParams,
    ) -> ResponseFuture<lsp_types::DocumentDiagnosticReportResult>;

    // Probably not the ideal ultimate design
    fn capabilities(&self) -> &lsp_types::ServerCapabilities;

    fn position_encoding(&self) -> PositionEncoding;
}

pub type ResponseFuture<T> = BoxFuture<'static, Result<T>>;
