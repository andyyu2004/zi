pub mod lstypes;

use std::any::Any;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use futures_core::future::BoxFuture;
use zi_text::Deltas;

use crate::syntax::Theme;
use crate::{Client, LanguageServiceId, MarkBuilder, Setting};

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

/// A client to the editor per language service.
pub struct LanguageClient {
    service_id: LanguageServiceId,
    client: Client,
}

impl Deref for LanguageClient {
    type Target = Client;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl LanguageClient {
    pub fn new(for_server: LanguageServiceId, client: Client) -> Self {
        Self { client, service_id: for_server }
    }

    /// The language service this client is associated with.
    pub fn service_id(&self) -> LanguageServiceId {
        self.service_id
    }
}

impl fmt::Debug for LanguageClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LanguageClient").field("server", &self.service_id).finish()
    }
}

/// An abstraction of language server requests. Notifications are handled via the event system.
pub trait LanguageService {
    fn as_any_mut(&mut self) -> &mut dyn Any;

    // TODO add more information than () as required
    fn definition_capabilities(&self) -> Option<()>;
    fn declaration_capabilities(&self) -> Option<()>;
    fn implementation_capabilities(&self) -> Option<()>;
    fn type_definition_capabilities(&self) -> Option<()>;
    fn completion_capabilities(&self) -> Option<()>;
    fn reference_capabilities(&self) -> Option<()>;
    fn diagnostic_capabilities(&self) -> Option<()>;
    fn semantic_tokens_capabilities(&self) -> Option<()>;
    fn formatting_capabilities(&self) -> Option<()>;

    /// Initialize the language service.
    /// This must be called before any other method and should only be called exactly once.
    fn initialize(&mut self, params: lstypes::InitializeParams) -> ResponseFuture<()>;

    fn initialized(&mut self) -> Result<()>;

    fn formatting(
        &mut self,
        params: lstypes::DocumentFormattingParams,
    ) -> ResponseFuture<Option<Deltas<'static>>>;

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
        params: lstypes::CompletionParams,
    ) -> ResponseFuture<lstypes::CompletionResponse>;

    fn semantic_tokens_full(
        &mut self,
        // Bit of a hack parameter, find another cleaner way
        theme: Setting<Theme>,
        params: lstypes::SemanticTokensParams,
    ) -> ResponseFuture<Option<Vec<MarkBuilder>>>;

    // fn semantic_tokens_full_delta(
    //     &mut self,
    //     params: lstypes::SemanticTokensDeltaParams,
    // ) -> ResponseFuture<Option<lstypes::SemanticTokensFullDeltaResult>>;

    fn document_diagnostic(
        &mut self,
        params: lstypes::DocumentDiagnosticParams,
    ) -> ResponseFuture<lstypes::DocumentDiagnosticReport>;

    fn shutdown(&mut self) -> ResponseFuture<()>;

    fn exit(&mut self) -> Result<()>;
}

pub trait LanguageServiceConfig {
    /// Spawn a new language service instance.
    /// Returns a boxed language service and a future to spawn to run the service.
    #[allow(clippy::type_complexity)]
    fn spawn(
        &self,
        cwd: &Path,
        client: LanguageClient,
    ) -> Result<(Box<dyn LanguageService + Send>, BoxFuture<'static, Result<()>>)>;
}
