use std::path::Path;

use anyhow::Result;
use futures_core::future::BoxFuture;
// TODO using lsp_types for now, but should define our own lsp-agnostic interface to drop the dependency;
pub use lsp_types;
pub use zi_core::PositionEncoding;

pub type ResponseFuture<T> = BoxFuture<'static, Result<T>>;

/// An abstraction of language server requests. Notifications are handled via the event system.
pub trait LanguageService {
    /// Initialize the language server.
    /// This must be called before any other method and should only be called exactly once.
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

    fn semantic_tokens_full_delta(
        &mut self,
        params: lsp_types::SemanticTokensDeltaParams,
    ) -> ResponseFuture<Option<lsp_types::SemanticTokensFullDeltaResult>>;

    fn document_diagnostic(
        &mut self,
        params: lsp_types::DocumentDiagnosticParams,
    ) -> ResponseFuture<lsp_types::DocumentDiagnosticReportResult>;

    fn capabilities(&self) -> &lsp_types::ServerCapabilities;

    fn position_encoding(&self) -> PositionEncoding;
}

/// A client to the editor for the language server.
pub trait LanguageClient: Send {
    fn log_message(&mut self, message: lsp_types::LogMessageParams);

    fn publish_diagnostics(&mut self, params: lsp_types::PublishDiagnosticsParams);
}

impl<C: LanguageClient + ?Sized> LanguageClient for Box<C> {
    #[inline]
    fn log_message(&mut self, message: lsp_types::LogMessageParams) {
        self.as_mut().log_message(message)
    }

    #[inline]
    fn publish_diagnostics(&mut self, params: lsp_types::PublishDiagnosticsParams) {
        self.as_mut().publish_diagnostics(params)
    }
}

pub trait LanguageServiceConfig {
    /// Spawn a new language service instance.
    /// Returns a boxed language service and a future to spawn to run the service.
    #[allow(clippy::type_complexity)]
    fn spawn(
        &self,
        cwd: &Path,
        client: Box<dyn LanguageClient>,
    ) -> Result<(Box<dyn LanguageService + Send>, BoxFuture<'static, Result<()>>)>;
}
