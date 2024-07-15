use std::marker::PhantomData;
use std::sync::{Arc, OnceLock};

use async_lsp::{lsp_types, LanguageServer};
use futures_util::{FutureExt, TryFutureExt};
use zi_event::Registry;
use zi_language_service::{LanguageService, PositionEncoding, ResponseFuture};

/// async_lsp::LanguageServer -> zi::LanguageService
pub struct ToLanguageService<S, E> {
    server: S,
    capabilities: Arc<OnceLock<lsp_types::ServerCapabilities>>,
    position_encoding: OnceLock<PositionEncoding>,
    _editor: PhantomData<E>,
}

impl<S, E> ToLanguageService<S, E> {
    pub fn new(server: S) -> Self {
        Self {
            server,
            capabilities: Default::default(),
            position_encoding: Default::default(),
            _editor: PhantomData,
        }
    }
}

impl<S, E> LanguageService<E> for ToLanguageService<S, E>
where
    S: LanguageServer<Error = async_lsp::Error>,
    E: Registry<E>,
{
    fn initialize(
        &mut self,
        params: lsp_types::InitializeParams,
    ) -> ResponseFuture<lsp_types::InitializeResult> {
        let caps = Arc::clone(&self.capabilities);
        let fut = self.server.initialize(params);
        Box::pin(async move {
            let result = fut.await?;
            caps.set(result.capabilities.clone()).expect("capabilities already initialized");
            Ok(result)
        })
    }

    fn initialized(&mut self, registry: &mut E) {
        // registry.subscribe()
    }

    fn formatting(
        &mut self,
        params: lsp_types::DocumentFormattingParams,
    ) -> ResponseFuture<Option<Vec<lsp_types::TextEdit>>> {
        self.server.formatting(params).map_err(Into::into).boxed()
    }

    fn definition(
        &mut self,
        params: lsp_types::GotoDefinitionParams,
    ) -> ResponseFuture<Option<lsp_types::GotoDefinitionResponse>> {
        self.server.definition(params).map_err(Into::into).boxed()
    }

    fn references(
        &mut self,
        params: lsp_types::ReferenceParams,
    ) -> ResponseFuture<Option<Vec<lsp_types::Location>>> {
        self.server.references(params).map_err(Into::into).boxed()
    }

    fn type_definition(
        &mut self,
        params: lsp_types::GotoDefinitionParams,
    ) -> ResponseFuture<Option<lsp_types::GotoDefinitionResponse>> {
        self.server.type_definition(params).map_err(Into::into).boxed()
    }

    fn implementation(
        &mut self,
        params: lsp_types::GotoDefinitionParams,
    ) -> ResponseFuture<Option<lsp_types::GotoDefinitionResponse>> {
        self.server.implementation(params).map_err(Into::into).boxed()
    }

    fn completion(
        &mut self,
        params: lsp_types::CompletionParams,
    ) -> ResponseFuture<Option<lsp_types::CompletionResponse>> {
        self.server.completion(params).map_err(Into::into).boxed()
    }

    fn semantic_tokens_full(
        &mut self,
        params: lsp_types::SemanticTokensParams,
    ) -> ResponseFuture<Option<lsp_types::SemanticTokensResult>> {
        self.server.semantic_tokens_full(params).map_err(Into::into).boxed()
    }

    fn semantic_tokens_full_delta(
        &mut self,
        params: lsp_types::SemanticTokensDeltaParams,
    ) -> ResponseFuture<Option<lsp_types::SemanticTokensFullDeltaResult>> {
        self.server.semantic_tokens_full_delta(params).map_err(Into::into).boxed()
    }

    fn document_diagnostic(
        &mut self,
        params: lsp_types::DocumentDiagnosticParams,
    ) -> ResponseFuture<lsp_types::DocumentDiagnosticReportResult> {
        self.server.document_diagnostic(params).map_err(Into::into).boxed()
    }

    fn capabilities(&self) -> &lsp_types::ServerCapabilities {
        self.capabilities.get().expect("capabilities not initialized")
    }

    fn position_encoding(&self) -> PositionEncoding {
        *self.position_encoding.get_or_init(|| match &Self::capabilities(self).position_encoding {
            Some(encoding) => match encoding {
                enc if *enc == lsp_types::PositionEncodingKind::UTF8 => PositionEncoding::Utf8,
                enc if *enc == lsp_types::PositionEncodingKind::UTF16 => PositionEncoding::Utf16,
                _ => {
                    tracing::warn!("server returned unknown position encoding: {encoding:?}",);
                    PositionEncoding::default()
                }
            },
            None => {
                tracing::warn!("server did not return position encoding, defaulting to UTF-16");
                PositionEncoding::default()
            }
        })
    }
}
