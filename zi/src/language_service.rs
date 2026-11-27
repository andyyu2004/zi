pub mod lstypes;

use std::any::Any;
use std::ffi::OsString;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use futures_core::future::BoxFuture;
use futures_util::FutureExt;
use zi_text::Deltas;

use crate::syntax::Theme;
use crate::{Client, FileType, LanguageServiceId, MarkBuilder, Setting};

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

    /// Whether the service's main loop is still running. It stops when the server exits or crashes.
    pub(crate) fn is_running(&self) -> bool {
        !self.handle.is_finished()
    }

    pub(crate) async fn wait(mut self) -> Result<()> {
        self.handle.abort();
        tokio::time::timeout(Duration::from_millis(50), &mut self.handle).await??
    }
}

impl Drop for LanguageServiceInstance {
    fn drop(&mut self) {
        self.handle.abort();
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
#[derive(Clone)]
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
        f.debug_struct("LanguageClient").field("server", &self.service_id).finish_non_exhaustive()
    }
}

/// An abstraction of language server requests. Notifications are handled via the event system.
#[expect(
    clippy::unimplemented,
    reason = "requests are only sent when the matching capability is set"
)]
pub trait LanguageService {
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// The server's name and version, once it has initialized.
    fn server_info(&self) -> Option<&lstypes::ServerInfo> {
        None
    }

    /// The process id of the server, if it runs as a separate process.
    fn process_id(&self) -> Option<u32> {
        None
    }

    fn position_encoding(&self) -> Option<lstypes::PositionEncoding> {
        None
    }

    // TODO add more information than () as required
    fn definition_capabilities(&self) -> Option<()> {
        None
    }

    fn declaration_capabilities(&self) -> Option<()> {
        None
    }

    fn implementation_capabilities(&self) -> Option<()> {
        None
    }

    fn type_definition_capabilities(&self) -> Option<()> {
        None
    }

    fn completion_capabilities(&self) -> Option<()> {
        None
    }

    fn reference_capabilities(&self) -> Option<()> {
        None
    }

    fn diagnostic_capabilities(&self) -> Option<()> {
        None
    }

    fn semantic_tokens_capabilities(&self) -> Option<()> {
        None
    }

    fn formatting_capabilities(&self) -> Option<()> {
        None
    }

    /// Initialize the language service.
    /// This must be called before any other method and should only be called exactly once.
    fn initialize(&mut self, params: lstypes::InitializeParams) -> ResponseFuture<()> {
        let _ = params;
        async { Ok(()) }.boxed()
    }

    fn initialized(&mut self) -> Result<()> {
        Ok(())
    }

    fn format(
        &mut self,
        params: lstypes::DocumentFormattingParams,
    ) -> ResponseFuture<Option<Deltas<'static>>> {
        let _ = params;
        unimplemented!()
    }

    fn definition(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse> {
        let _ = params;
        unimplemented!()
    }

    fn type_definition(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse> {
        let _ = params;
        unimplemented!()
    }

    fn implementation(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse> {
        let _ = params;
        unimplemented!()
    }

    fn references(
        &mut self,
        params: lstypes::ReferenceParams,
    ) -> ResponseFuture<Vec<lstypes::Location>> {
        let _ = params;
        unimplemented!()
    }

    fn completion(
        &mut self,
        params: lstypes::CompletionParams,
    ) -> ResponseFuture<lstypes::CompletionResponse> {
        let _ = params;
        unimplemented!()
    }

    fn semantic_tokens_full(
        &mut self,
        // Bit of a hack parameter, find another cleaner way
        theme: Setting<Theme>,
        params: lstypes::SemanticTokensParams,
    ) -> ResponseFuture<Option<Vec<MarkBuilder>>> {
        let _ = params;
        let _ = theme;
        unimplemented!()
    }

    // fn semantic_tokens_full_delta(
    //     &mut self,
    //     params: lstypes::SemanticTokensDeltaParams,
    // ) -> ResponseFuture<Option<lstypes::SemanticTokensFullDeltaResult>>;

    fn document_diagnostic(
        &mut self,
        params: lstypes::DocumentDiagnosticParams,
    ) -> ResponseFuture<lstypes::DocumentDiagnosticReport> {
        let _ = params;
        unimplemented!()
    }

    fn shutdown(&mut self) -> ResponseFuture<()> {
        async { Ok(()) }.boxed()
    }

    fn exit(&mut self) -> Result<()> {
        Ok(())
    }
}

/// The program and arguments a language service is started with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceCommand {
    pub program: OsString,
    pub args: Box<[OsString]>,
}

bitflags::bitflags! {
    /// The requests a language service said it supports when it initialized.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ServiceCapabilities: u16 {
        const DEFINITION = 1 << 0;
        const DECLARATION = 1 << 1;
        const IMPLEMENTATION = 1 << 2;
        const TYPE_DEFINITION = 1 << 3;
        const REFERENCES = 1 << 4;
        const COMPLETION = 1 << 5;
        const DIAGNOSTICS = 1 << 6;
        const SEMANTIC_TOKENS = 1 << 7;
        const FORMATTING = 1 << 8;
    }
}

/// What the editor knows about a configured language service. See [`crate::Editor::language_services`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageServiceStatus {
    pub id: LanguageServiceId,
    pub state: LanguageServiceState,
    /// `None` if the service is not defined or does not run as a separate process.
    pub command: Option<ServiceCommand>,
    /// The file types configured to use this service.
    pub filetypes: Box<[FileType]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageServiceState {
    /// Initialized and its main loop is running.
    Running(ServiceDetails),
    /// Initialized, but its main loop has since stopped: the server exited or crashed.
    Exited(ServiceDetails),
    /// Spawned and waiting for `initialize` to return.
    Starting,
    /// The last attempt to start it failed, with this error.
    Failed(String),
    /// Defined, but no buffer of its file types has been opened yet.
    NotStarted,
    /// A language uses it, but it has no definition.
    Undefined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDetails {
    pub server_info: Option<lstypes::ServerInfo>,
    pub process_id: Option<u32>,
    pub root: PathBuf,
    pub position_encoding: Option<lstypes::PositionEncoding>,
    pub capabilities: ServiceCapabilities,
}

pub trait LanguageServiceConfig {
    /// The command this service runs, if it runs as a separate process.
    fn command(&self) -> Option<ServiceCommand> {
        None
    }

    /// Spawn a new language service instance.
    /// Returns a boxed language service and a future to spawn to run the service.
    #[expect(clippy::type_complexity, reason = "spelled out once, here")]
    fn spawn(
        &self,
        cwd: &Path,
        client: LanguageClient,
    ) -> Result<(Box<dyn LanguageService + Send>, BoxFuture<'static, Result<()>>)>;
}
