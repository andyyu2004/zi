#[doc(hidden)]
pub mod from_proto;
#[doc(hidden)]
pub mod to_proto;

use std::future::ready;
use std::ops::{ControlFlow, Deref, DerefMut};
use std::time::Duration;

use futures_core::future::BoxFuture;
use zi_lsp::lsp_types::notification::Notification;
use zi_lsp::lsp_types::request::Request;
use zi_lsp::lsp_types::{self, lsp_notification, lsp_request, ClientCapabilities};
use zi_lsp::{ErrorCode, PositionEncoding, ResponseError, Result};

use crate::{Client, LanguageServerId};

pub struct LanguageClient {
    for_server: LanguageServerId,
    client: Client,
}

impl LanguageClient {
    pub fn new(for_server: LanguageServerId, client: Client) -> Self {
        Self { client, for_server }
    }
}

type ResponseFuture<R, E> = BoxFuture<'static, Result<<R as Request>::Result, E>>;

fn method_not_found<R, E>() -> ResponseFuture<R, E>
where
    R: Request,
    R::Result: Send + 'static,
    E: From<ResponseError> + Send + 'static,
{
    Box::pin(ready(Err(ResponseError::new(
        ErrorCode::METHOD_NOT_FOUND,
        format!("No such method: {}", R::METHOD),
    )
    .into())))
}

impl zi_lsp::LanguageClient for LanguageClient {
    type Error = ResponseError;

    type NotifyResult = ControlFlow<Result<()>>;

    #[must_use]
    fn workspace_folders(
        &mut self,
        _params: <lsp_request!("workspace/workspaceFolders") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("workspace/workspaceFolders"), Self::Error> {
        method_not_found::<lsp_request!("workspace/workspaceFolders"), _>()
    }

    #[must_use]
    fn configuration(
        &mut self,
        params: <lsp_request!("workspace/configuration") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("workspace/configuration"), Self::Error> {
        let _ = params;
        method_not_found::<lsp_request!("workspace/configuration"), _>()
    }

    #[must_use]
    fn work_done_progress_create(
        &mut self,
        _params: <lsp_request!("window/workDoneProgress/create") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("window/workDoneProgress/create"), Self::Error> {
        method_not_found::<lsp_request!("window/workDoneProgress/create"), _>()
    }

    #[must_use]
    fn semantic_tokens_refresh(
        &mut self,
        _params: <lsp_request!("workspace/semanticTokens/refresh") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("workspace/semanticTokens/refresh"), Self::Error> {
        method_not_found::<lsp_request!("workspace/semanticTokens/refresh"), _>()
    }

    #[must_use]
    fn show_document(
        &mut self,
        _params: <lsp_request!("window/showDocument") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("window/showDocument"), Self::Error> {
        method_not_found::<lsp_request!("window/showDocument"), _>()
    }

    #[must_use]
    fn inline_value_refresh(
        &mut self,
        _params: <lsp_request!("workspace/inlineValue/refresh") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("workspace/inlineValue/refresh"), Self::Error> {
        method_not_found::<lsp_request!("workspace/inlineValue/refresh"), _>()
    }

    #[must_use]
    fn inlay_hint_refresh(
        &mut self,
        _params: <lsp_request!("workspace/inlayHint/refresh") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("workspace/inlayHint/refresh"), Self::Error> {
        method_not_found::<lsp_request!("workspace/inlayHint/refresh"), _>()
    }

    #[must_use]
    fn workspace_diagnostic_refresh(
        &mut self,
        _params: <lsp_request!("workspace/diagnostic/refresh") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("workspace/diagnostic/refresh"), Self::Error> {
        method_not_found::<lsp_request!("workspace/diagnostic/refresh"), _>()
    }

    #[must_use]
    fn register_capability(
        &mut self,
        params: <lsp_request!("client/registerCapability") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("client/registerCapability"), Self::Error> {
        let _ = params;
        method_not_found::<lsp_request!("client/registerCapability"), _>()
    }

    #[must_use]
    fn unregister_capability(
        &mut self,
        params: <lsp_request!("client/unregisterCapability") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("client/unregisterCapability"), Self::Error> {
        let _ = params;
        method_not_found::<lsp_request!("client/unregisterCapability"), _>()
    }

    #[must_use]
    fn show_message_request(
        &mut self,
        params: <lsp_request!("window/showMessageRequest") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("window/showMessageRequest"), Self::Error> {
        let _ = params;
        method_not_found::<lsp_request!("window/showMessageRequest"), _>()
    }

    #[must_use]
    fn code_lens_refresh(
        &mut self,
        _params: <lsp_request!("workspace/codeLens/refresh") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("workspace/codeLens/refresh"), Self::Error> {
        method_not_found::<lsp_request!("workspace/codeLens/refresh"), _>()
    }

    #[must_use]
    fn apply_edit(
        &mut self,
        params: <lsp_request!("workspace/applyEdit") as Request>::Params,
    ) -> ResponseFuture<lsp_request!("workspace/applyEdit"), Self::Error> {
        let _ = params;
        method_not_found::<lsp_request!("workspace/applyEdit"), _>()
    }

    #[must_use]
    fn show_message(
        &mut self,
        params: <lsp_notification!("window/showMessage") as Notification>::Params,
    ) -> Self::NotifyResult {
        let _ = params;
        ControlFlow::Continue(())
    }

    #[must_use]
    fn log_message(
        &mut self,
        params: <lsp_notification!("window/logMessage") as Notification>::Params,
    ) -> Self::NotifyResult {
        let _ = params;
        ControlFlow::Continue(())
    }

    #[must_use]
    fn telemetry_event(
        &mut self,
        params: <lsp_notification!("telemetry/event") as Notification>::Params,
    ) -> Self::NotifyResult {
        let _ = params;
        ControlFlow::Continue(())
    }

    #[must_use]
    fn publish_diagnostics(
        &mut self,
        params: <lsp_notification!("textDocument/publishDiagnostics") as Notification>::Params,
    ) -> Self::NotifyResult {
        let server = self.for_server;
        self.client.send(move |editor| {
            let Ok(path) = params.uri.to_file_path() else {
                tracing::warn!("received diagnostics for non-file URI: {}", params.uri);
                return Ok(());
            };
            editor.update_diagnostics(server, path, params.diagnostics);
            Ok(())
        });

        ControlFlow::Continue(())
    }

    #[must_use]
    fn log_trace(
        &mut self,
        params: <lsp_notification!("$/logTrace") as Notification>::Params,
    ) -> Self::NotifyResult {
        let _ = params;
        ControlFlow::Continue(())
    }

    #[must_use]
    fn cancel_request(
        &mut self,
        params: <lsp_notification!("$/cancelRequest") as Notification>::Params,
    ) -> Self::NotifyResult {
        let _ = params;
        ControlFlow::Continue(())
    }

    #[must_use]
    fn progress(
        &mut self,
        params: <lsp_notification!("$/progress") as Notification>::Params,
    ) -> Self::NotifyResult {
        let _ = params;
        ControlFlow::Continue(())
    }
}

pub(crate) struct LanguageServer {
    pub capabilities: lsp_types::ServerCapabilities,
    handle: tokio::task::JoinHandle<zi_lsp::Result<()>>,
    // Storing this odd type to allow for a test implementation.
    // The `DerefMut` is useful to make it easy to delegate the actual server implementation to an inner type.
    server: Box<dyn DerefMut<Target = zi_lsp::DynLanguageServer> + Send>,
    position_encoding: PositionEncoding,
}

impl LanguageServer {
    pub(crate) fn new(
        capabilities: lsp_types::ServerCapabilities,
        handle: tokio::task::JoinHandle<zi_lsp::Result<()>>,
        server: Box<dyn DerefMut<Target = zi_lsp::DynLanguageServer> + Send>,
    ) -> Self {
        let position_encoding = match &capabilities.position_encoding {
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
        };

        Self { capabilities, handle, server, position_encoding }
    }

    pub(crate) fn position_encoding(&self) -> PositionEncoding {
        self.position_encoding
    }

    /// Wait for the language server to finish.
    /// This assumes that `shutdown` has been requested.
    pub(crate) async fn wait(self) -> crate::Result<()> {
        self.handle.abort();
        Ok(tokio::time::timeout(Duration::from_millis(50), self.handle).await???)
    }
}

impl Deref for LanguageServer {
    type Target = zi_lsp::DynLanguageServer;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &*self.server
    }
}

impl DerefMut for LanguageServer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.server
    }
}

pub fn client_capabilities() -> ClientCapabilities {
    const GOTO_CAPABILITY: Option<lsp_types::GotoCapability> = Some(lsp_types::GotoCapability {
        dynamic_registration: Some(false),
        link_support: Some(true),
    });

    lsp_types::ClientCapabilities {
        workspace: None,
        text_document: Some(lsp_types::TextDocumentClientCapabilities {
            declaration: GOTO_CAPABILITY,
            definition: GOTO_CAPABILITY,
            type_definition: GOTO_CAPABILITY,
            implementation: GOTO_CAPABILITY,
            publish_diagnostics: Some(lsp_types::PublishDiagnosticsClientCapabilities {
                version_support: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        }),
        window: None,
        general: Some(lsp_types::GeneralClientCapabilities {
            position_encodings: Some(vec![
                // There are in order of preference
                lsp_types::PositionEncodingKind::UTF8,
                lsp_types::PositionEncodingKind::UTF16,
                // No support for UTF32 yet
            ]),
            ..Default::default()
        }),
        experimental: None,
    }
}
