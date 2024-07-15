pub mod lstypes;

use std::any::Any;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use futures_core::future::BoxFuture;
// TODO using lsp_types for now, but should define our own lsp-agnostic interface to drop the dependency;
pub use lsp_types;
pub use zi_core::PositionEncoding;

use crate::LanguageServiceId;

pub type ResponseFuture<T> = BoxFuture<'static, Result<T>>;

pub struct LanguageServiceInstance {
    service: Box<dyn LanguageService + Send>,
    handle: tokio::task::JoinHandle<Result<()>>,
}

impl LanguageServiceInstance {
    pub(crate) fn new(
        service: Box<dyn LanguageService + Send>,
        handle: tokio::task::JoinHandle<Result<()>>,
    ) -> Self {
        Self { service, handle }
    }

    pub(crate) async fn wait(self) -> Result<()> {
        self.handle.abort();
        Ok(tokio::time::timeout(Duration::from_millis(50), self.handle).await???)
    }
}

impl Deref for LanguageServiceInstance {
    type Target = dyn LanguageService + Send;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &*self.service
    }
}

impl DerefMut for LanguageServiceInstance {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.service
    }
}

/// An abstraction of language server requests. Notifications are handled via the event system.
pub trait LanguageService {
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Initialize the language service.
    /// This must be called before any other method and should only be called exactly once.
    fn initialize(&mut self, params: lstypes::InitializeParams) -> ResponseFuture<()>;

    fn initialized(&mut self) -> Result<()>;

    fn formatting(
        &mut self,
        params: lstypes::DocumentFormattingParams,
    ) -> ResponseFuture<Option<Vec<lsp_types::TextEdit>>>;

    fn definition(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse>;

    fn type_definition(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse>;

    fn implementation(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse>;

    fn references(
        &mut self,
        params: lstypes::ReferenceParams,
    ) -> ResponseFuture<Vec<lstypes::Location>>;

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

    fn shutdown(&mut self) -> ResponseFuture<()>;

    fn exit(&mut self) -> Result<()>;
}

/// A client to the editor for the language server.
pub trait LanguageClient: Send {
    /// The service this client is associated with.
    fn service_id(&self) -> LanguageServiceId;

    fn log_message(&mut self, message: lsp_types::LogMessageParams);

    fn publish_diagnostics(&mut self, params: lsp_types::PublishDiagnosticsParams);
}

impl<C: LanguageClient + ?Sized> LanguageClient for Box<C> {
    #[inline]
    fn service_id(&self) -> LanguageServiceId {
        self.as_ref().service_id()
    }

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
