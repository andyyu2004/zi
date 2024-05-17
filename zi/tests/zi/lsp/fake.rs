use std::collections::HashMap;
use std::future::ready;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;

use futures_core::future::BoxFuture;
use futures_core::Future;
use serde_json::Value;
use zi_lsp::lsp_types::notification::Notification;
use zi_lsp::lsp_types::request::Request;
use zi_lsp::lsp_types::{lsp_notification, lsp_request};
use zi_lsp::{DynLanguageServer, ErrorCode, LanguageServer, ResponseError, Result};

pub struct FakeLanguageServerBuilder<St> {
    handlers: Handlers<St>,
}

impl<St> Default for FakeLanguageServerBuilder<St> {
    fn default() -> Self {
        Self { handlers: Default::default() }
    }
}

pub struct FakeLanguageServerTemplate<St> {
    init_state: St,
    handlers: Arc<Handlers<St>>,
}

impl<St: Default> Default for FakeLanguageServerTemplate<St> {
    fn default() -> Self {
        Self { init_state: Default::default(), handlers: Default::default() }
    }
}

impl<St: Clone + Send + 'static> zi::LanguageServerConfig for FakeLanguageServerTemplate<St> {
    fn spawn(
        &self,
    ) -> zi_lsp::Result<Box<dyn DerefMut<Target = zi_lsp::DynLanguageServer> + Send>> {
        Ok(Box::new(FakeLanguageServer {
            handlers: Arc::clone(&self.handlers),
            state: self.init_state.clone(),
        }))
    }
}

impl<St> Default for Handlers<St> {
    fn default() -> Self {
        Self { reqs: HashMap::new(), notifs: HashMap::new() }
    }
}

struct Handlers<St> {
    reqs: HashMap<&'static str, BoxReqHandler<St, zi_lsp::Error>>,
    notifs: HashMap<&'static str, BoxNotifHandler<St>>,
}

impl<St> FakeLanguageServerBuilder<St> {
    pub fn finish(self, init_state: St) -> FakeLanguageServerTemplate<St> {
        FakeLanguageServerTemplate { init_state, handlers: Arc::new(self.handlers) }
    }

    pub fn request<R: Request, Fut>(
        mut self,
        handler: impl Fn(&mut St, R::Params) -> Fut + Send + Sync + 'static,
    ) -> Self
    where
        Fut: Future<Output = Result<R::Result, zi_lsp::Error>> + Send + 'static,
    {
        tracing::info!("registering request handler for {}", R::METHOD);
        self.handlers.reqs.insert(
            R::METHOD,
            Box::new(move |state, req| match serde_json::from_value::<R::Params>(req.params) {
                Ok(params) => {
                    let fut = handler(state, params);
                    Box::pin(async move {
                        Ok(serde_json::to_value(fut.await?).expect("serialization failed"))
                    })
                }
                Err(err) => Box::pin(ready(Err(ResponseError::new(
                    ErrorCode::INVALID_PARAMS,
                    format!("failed to deserialize parameters: {err}"),
                )
                .into()))),
            }),
        );
        self
    }

    #[allow(dead_code)]
    pub fn notification<N: Notification>(
        mut self,
        handler: impl Fn(&mut St, N::Params) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        tracing::info!("registering notification handler for {}", N::METHOD);
        self.handlers.notifs.insert(
            N::METHOD,
            Box::new(move |state, notif| match serde_json::from_value::<N::Params>(notif.params) {
                Ok(params) => handler(state, params),
                Err(err) => Err(err.into()),
            }),
        );
        self
    }
}

pub struct FakeLanguageServer<St> {
    state: St,
    handlers: Arc<Handlers<St>>,
}

struct AnyRequest {
    params: Value,
}

struct AnyNotification {
    params: Value,
}

type BoxReqFuture<Error> = Pin<Box<dyn Future<Output = Result<Value, Error>> + Send>>;
type BoxReqHandler<St, Error> =
    Box<dyn Fn(&mut St, AnyRequest) -> BoxReqFuture<Error> + Send + Sync>;
type BoxNotifHandler<St> = Box<dyn Fn(&mut St, AnyNotification) -> Result<()> + Send + Sync>;

impl<St> FakeLanguageServer<St> {
    pub fn builder() -> FakeLanguageServerBuilder<St> {
        FakeLanguageServerBuilder::default()
    }

    pub fn request<R: Request>(&mut self, params: R::Params) -> ResponseFuture<R> {
        tracing::info!("request: {}", R::METHOD);
        let f = self
            .handlers
            .reqs
            .get(R::METHOD)
            .unwrap_or_else(|| panic!("missing request handler: {}", R::METHOD));
        let req = AnyRequest { params: serde_json::to_value(params).expect("failed to serialize") };
        let res = f(&mut self.state, req);
        Box::pin(async move {
            let x = res.await?;
            Ok(serde_json::from_value(x).expect("failed to deserialize"))
        })
    }

    pub fn notify<N: Notification>(&mut self, params: N::Params) -> Result<()> {
        tracing::info!("notification: {}", N::METHOD);
        let f = self
            .handlers
            .notifs
            .get(N::METHOD)
            .unwrap_or_else(|| panic!("missing notification handler: {}", N::METHOD));
        let notif =
            AnyNotification { params: serde_json::to_value(params).expect("failed to serialize") };

        f(&mut self.state, notif)
    }
}

type ResponseFuture<R, E = zi_lsp::Error> = BoxFuture<'static, Result<<R as Request>::Result, E>>;

macro_rules! requests {
    ({ $($req_server:tt, $req_server_snake:ident;)* }) => {
        $(
            fn $req_server_snake(&mut self, params: <lsp_request!($req_server) as Request>::Params) -> ResponseFuture<lsp_request!($req_server), Self::Error> {
                self.request::<lsp_request!($req_server)>(params)
            }
        )*
    };
}

macro_rules! notifications {
    ({ $($req_server:tt, $req_server_snake:ident;)* }) => {
        $(
            fn $req_server_snake(&mut self, params: <lsp_notification!($req_server) as Notification>::Params) -> Result<()> {
                self.notify::<lsp_notification!($req_server)>(params)
            }
        )*
    };
}

impl<St> LanguageServer for FakeLanguageServer<St> {
    type Error = zi_lsp::Error;

    type NotifyResult = Result<(), Self::Error>;

    requests!({
        "initialize", initialize;
        "shutdown", shutdown;

        "textDocument/implementation", implementation;
        "textDocument/typeDefinition", type_definition;
        "textDocument/documentColor", document_color;
        "textDocument/colorPresentation", color_presentation;
        "textDocument/foldingRange", folding_range;
        "textDocument/declaration", declaration;
        "textDocument/selectionRange", selection_range;
        "textDocument/prepareCallHierarchy", prepare_call_hierarchy;
        "callHierarchy/incomingCalls", incoming_calls;
        "callHierarchy/outgoingCalls", outgoing_calls;
        "textDocument/semanticTokens/full", semantic_tokens_full;
        "textDocument/semanticTokens/full/delta", semantic_tokens_full_delta;
        "textDocument/semanticTokens/range", semantic_tokens_range;
        "textDocument/linkedEditingRange", linked_editing_range;
        "workspace/willCreateFiles", will_create_files;
        "workspace/willRenameFiles", will_rename_files;
        "workspace/willDeleteFiles", will_delete_files;
        "textDocument/moniker", moniker;
        "textDocument/prepareTypeHierarchy", prepare_type_hierarchy;
        "typeHierarchy/supertypes", supertypes;
        "typeHierarchy/subtypes", subtypes;
        "textDocument/inlineValue", inline_value;
        "textDocument/inlayHint", inlay_hint;
        "inlayHint/resolve", inlay_hint_resolve;
        "textDocument/diagnostic", document_diagnostic;
        "workspace/diagnostic", workspace_diagnostic;
        "textDocument/willSaveWaitUntil", will_save_wait_until;
        "textDocument/completion", completion;
        "completionItem/resolve", completion_item_resolve;
        "textDocument/hover", hover;
        "textDocument/signatureHelp", signature_help;
        "textDocument/definition", definition;
        "textDocument/references", references;
        "textDocument/documentHighlight", document_highlight;
        "textDocument/documentSymbol", document_symbol;
        "textDocument/codeAction", code_action;
        "codeAction/resolve", code_action_resolve;
        "workspace/symbol", symbol;
        "workspaceSymbol/resolve", workspace_symbol_resolve;
        "textDocument/codeLens", code_lens;
        "codeLens/resolve", code_lens_resolve;
        "textDocument/documentLink", document_link;
        "documentLink/resolve", document_link_resolve;
        "textDocument/formatting", formatting;
        "textDocument/rangeFormatting", range_formatting;
        "textDocument/onTypeFormatting", on_type_formatting;
        "textDocument/rename", rename;
        "textDocument/prepareRename", prepare_rename;
        "workspace/executeCommand", execute_command;
    });

    notifications!({
        "initialized", initialized;
        "exit", exit;

        "workspace/didChangeWorkspaceFolders", did_change_workspace_folders;
        "window/workDoneProgress/cancel", work_done_progress_cancel;
        "workspace/didCreateFiles", did_create_files;
        "workspace/didRenameFiles", did_rename_files;
        "workspace/didDeleteFiles", did_delete_files;
        "workspace/didChangeConfiguration", did_change_configuration;
        "textDocument/didOpen", did_open;
        "textDocument/didChange", did_change;
        "textDocument/didClose", did_close;
        "textDocument/didSave", did_save;
        "textDocument/willSave", will_save;
        "workspace/didChangeWatchedFiles", did_change_watched_files;
        "$/setTrace", set_trace;
        "$/cancelRequest", cancel_request;
        "$/progress", progress;
    });
}

impl<St: 'static> Deref for FakeLanguageServer<St> {
    type Target = DynLanguageServer;

    fn deref(&self) -> &Self::Target {
        self
    }
}

impl<St: 'static> DerefMut for FakeLanguageServer<St> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self
    }
}
