use std::future::ready;
use std::ops::ControlFlow;

use async_lsp::lsp_types::notification::Notification;
use async_lsp::lsp_types::request::Request;
use async_lsp::lsp_types::{lsp_notification, lsp_request};
use async_lsp::{ErrorCode, ResponseError};
use futures_util::future::BoxFuture;
use zi::LanguageService;

use crate::{from_proto, EditorExt};

/// Adaptor to convert zi::LanguageClient -> async_lsp::LanguageClient
pub struct LanguageClient(zi::LanguageClient);

impl LanguageClient {
    pub fn new(client: zi::LanguageClient) -> Self {
        Self(client)
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

impl async_lsp::LanguageClient for LanguageClient {
    type Error = ResponseError;

    type NotifyResult = ControlFlow<async_lsp::Result<()>>;

    #[must_use]
    fn publish_diagnostics(
        &mut self,
        params: <lsp_notification!("textDocument/publishDiagnostics") as Notification>::Params,
    ) -> Self::NotifyResult {
        let service_id = self.0.service_id();
        self.0.send(move |editor| {
            let Some(service) = editor.language_server(service_id) else { return Ok(()) };
            let Some(text) = service.text(&params.uri) else { return Ok(()) };
            let encoding = service.position_encoding();
            let diagnostics = from_proto::diagnostics(encoding, text, params.diagnostics);

            let Ok(path) = params.uri.to_file_path() else {
                tracing::warn!("received diagnostics for non-file URI: {}", params.uri);
                return Ok(());
            };

            tracing::info!(
                %service_id,
                ?path,
                version = params.version,
                n = diagnostics.len(),
                "received push diagnostics"
            );

            editor.replace_diagnostics(
                path,
                params.version.map(|i| i as u32),
                diagnostics.into_boxed_slice(),
            );

            Ok(())
        });
        ControlFlow::Continue(())
    }

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
        self.0.send(move |editor| {
            tracing::info!("received log message");
            // TODO there are multiple levels of log messages
            editor.set_error(params.message);
            Ok(())
        });
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
