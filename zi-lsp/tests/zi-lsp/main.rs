use core::fmt;
use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{self, AtomicUsize};
use std::sync::Arc;

use futures_util::future::BoxFuture;
use zi_lsp::lsp_types::notification::{self, Notification};
use zi_lsp::lsp_types::request::{self, Request};
use zi_lsp::lsp_types::{self, lsp_notification, lsp_request, OneOf};
use zi_lsp::{ErrorCode, ResponseError, Result};
use zi_test::{new, TestContext};

mod definition;
mod diagnostics;
mod format;
mod sync;

// Utility type that can be referenced from within `Fn` closures.
pub struct ExpectedSequence<T> {
    xs: Vec<T>,
    idx: AtomicUsize,
}

impl<T> ExpectedSequence<T> {
    pub fn new(xs: impl Into<Vec<T>>) -> Self {
        Self { xs: xs.into(), idx: AtomicUsize::new(0) }
    }

    #[track_caller]
    pub fn assert_eq(&self, expected: &T)
    where
        T: fmt::Debug + PartialEq,
    {
        let idx = self.idx.fetch_add(1, atomic::Ordering::Relaxed);
        let actual = self.xs.get(idx).unwrap_or_else(|| {
            panic!("ExpectedSequence: out of bounds access at index {idx}, expected {expected:?}")
        });

        assert_eq!(actual, expected, "ExpectedSequence: mismatch at index {idx}");
    }
}

impl<T> Drop for ExpectedSequence<T> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            let idx = self.idx.load(atomic::Ordering::Relaxed);
            assert_eq!(
                idx,
                self.xs.len(),
                "ExpectedSequence: not all expected events were received"
            );
        }
    }
}

macro_rules! lsp_pos {
    ($line:literal:$character:literal) => {
        lsp_types::Position { line: $line, character: $character }
    };
}

macro_rules! lsp_range {
    ($start_line:literal:$start_character:literal..$end_line:literal:$end_character:literal) => {
        lsp_types::Range {
            start: lsp_pos!($start_line:$start_character),
            end: lsp_pos!($end_line:$end_character),
        }
    };
}

macro_rules! lsp_change_event {
    ($start_line:literal:$start_character:literal..$end_line:literal:$end_character:literal =>$text:expr) => {
        lsp_types::TextDocumentContentChangeEvent {
            range: Some(lsp_range!($start_line:$start_character..$end_line:$end_character)),
            text: $text.to_string(),
            range_length: None,
        }
    };
    ($text:expr) => {
        lsp_types::TextDocumentContentChangeEvent {
            range: None,
            text: $text.to_string(),
            range_length: None,
        }
    };
}

use {lsp_change_event, lsp_pos, lsp_range};

trait TestContextExt {
    async fn setup_lang_server<St: Send + Sync + Clone + 'static>(
        &self,
        ft: zi::FileType,
        server_id: impl Into<zi::LanguageServiceId>,
        st: St,
        f: impl FnOnce(FakeLanguageServerBuilder<St>) -> FakeLanguageServerBuilder<St>,
    );
}

impl TestContextExt for TestContext {
    async fn setup_lang_server<St: Send + Sync + Clone + 'static>(
        &self,
        ft: zi::FileType,
        server_id: impl Into<zi::LanguageServiceId>,
        st: St,
        f: impl FnOnce(FakeLanguageServerBuilder<St>) -> FakeLanguageServerBuilder<St>,
    ) {
        let server_id = server_id.into();
        // Setup a few default handlers.
        let server = f(FakeLanguageServer::builder()
            .request::<request::Initialize, _>(|_, _| async {
                Ok(lsp_types::InitializeResult::default())
            })
            .notification::<notification::Initialized>(|_st, _params| Ok(()))
            .notification::<notification::DidOpenTextDocument>(|_st, _params| Ok(()))
            .notification::<notification::DidChangeTextDocument>(|_st, _params| Ok(())));

        self.with(move |editor| {
            editor
                .language_config_mut()
                .add_language(ft, zi::LanguageConfig::new([server_id]))
                .add_language_service(server_id, server.finish(st));
        })
        .await
    }
}

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

impl<St: Clone + Send + Sync + 'static> zi::LanguageServiceConfig
    for FakeLanguageServerTemplate<St>
{
    fn spawn(
        &self,
        _cwd: &Path,
        client: zi::LanguageClient,
    ) -> anyhow::Result<(Box<dyn zi::LanguageService + Send>, BoxFuture<'static, anyhow::Result<()>>)>
    {
        let server = FakeLanguageServer {
            handlers: Arc::clone(&self.handlers),
            state: self.init_state.clone(),
        };

        Ok((Box::new(zi_lsp::LanguageService::new(client, server)), Box::pin(async { Ok(()) })))
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
        Fut: Future<Output = Result<R::Result>> + Send + 'static,
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
                Err(err) => Box::pin(std::future::ready(Err(ResponseError::new(
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
        handler: impl Fn(&mut St, N::Params) -> zi_lsp::Result<()> + Send + Sync + 'static,
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
    params: serde_json::Value,
}

struct AnyNotification {
    params: serde_json::Value,
}

type BoxReqFuture<Error> = Pin<Box<dyn Future<Output = Result<serde_json::Value, Error>> + Send>>;
type BoxReqHandler<St, Error> =
    Box<dyn Fn(&mut St, AnyRequest) -> BoxReqFuture<Error> + Send + Sync>;
type BoxNotifHandler<St> =
    Box<dyn Fn(&mut St, AnyNotification) -> zi_lsp::Result<()> + Send + Sync>;

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

impl<St> async_lsp::LanguageServer for FakeLanguageServer<St> {
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
