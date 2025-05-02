mod client;
#[doc(hidden)]
pub mod from_proto;
mod server;
#[doc(hidden)]
pub mod to_proto;

use std::ffi::{OsStr, OsString};
use std::future::Future;
use std::io;
use std::ops::ControlFlow;
use std::path::Path;
use std::process::Stdio;

use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router::Router;
use async_lsp::tracing::TracingLayer;
pub use async_lsp::{
    Error, ErrorCode, LanguageServer, ResponseError, Result, ServerSocket, lsp_types,
};
use futures_util::TryFutureExt;
use futures_util::future::BoxFuture;
use tokio::io::AsyncWriteExt;
use tokio_util::compat::FuturesAsyncReadCompatExt as _;
use tower::ServiceBuilder;

use self::client::LanguageClient;
pub use self::server::LanguageService;

pub fn start<C>(
    client: C,
    cwd: impl AsRef<Path>,
    cmd: impl AsRef<OsStr>,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<(ServerSocket, impl Future<Output = Result<()>> + 'static)>
where
    C: async_lsp::LanguageClient<
            NotifyResult = ControlFlow<crate::Result<()>>,
            Error = ResponseError,
        > + Send
        + 'static,
{
    let (main_loop, server) = async_lsp::MainLoop::new_client(|_server| {
        ServiceBuilder::new()
            .layer(TracingLayer::default())
            .layer(CatchUnwindLayer::default())
            .layer(ConcurrencyLayer::default())
            .service(Router::from_language_client(client))
    });

    let cmd = cmd.as_ref();
    let cwd = cwd.as_ref();
    let mut child = async_process::Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(false)
        .spawn()?;

    tracing::info!(?cmd, ?cwd, pid = child.id(), "spawned language server");

    let stdout = child.stdout.take().unwrap();
    let stdin = child.stdin.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    Ok((server, async move {
        // write stderr to a file /tmp/zi-lsp-log
        tokio::spawn(async move {
            let file = tokio::fs::File::create("/tmp/zi-lsp-log").await?;
            let mut writer = tokio::io::BufWriter::new(file);
            let mut reader = tokio::io::BufReader::new(stderr.compat());
            tokio::io::copy(&mut reader, &mut writer).await?;
            writer.flush().await?;
            Ok::<_, io::Error>(())
        });

        main_loop.run_buffered(stdout, stdin).await
    }))
}

#[derive(Debug)]
pub struct LanguageServerConfig {
    pub command: OsString,
    pub args: Box<[OsString]>,
}

impl LanguageServerConfig {
    pub fn new(command: impl Into<OsString>, args: impl IntoIterator<Item = OsString>) -> Self {
        Self { command: command.into(), args: args.into_iter().collect() }
    }
}

impl zi::LanguageServiceConfig for LanguageServerConfig {
    fn spawn(
        &self,
        cwd: &Path,
        client: zi::LanguageClient,
    ) -> anyhow::Result<(Box<dyn zi::LanguageService + Send>, BoxFuture<'static, anyhow::Result<()>>)>
    {
        tracing::debug!(command = ?self.command, args = ?self.args, "spawn language server");
        let (server, fut) =
            start(LanguageClient::new(client.clone()), cwd, &self.command, &self.args[..])?;
        Ok((Box::new(LanguageService::new(client, server)), Box::pin(fut.map_err(Into::into))))
    }
}

trait EditorExt {
    fn language_server(&mut self, service: zi::LanguageServiceId) -> Option<&mut LanguageService>;
}

impl EditorExt for zi::Editor {
    fn language_server(&mut self, service: zi::LanguageServiceId) -> Option<&mut LanguageService> {
        self.language_service(service).map(downcast_mut)
    }
}

fn downcast_mut<'a>(
    service: &'a mut (dyn zi::LanguageService + Send + 'static),
) -> &'a mut LanguageService {
    service.as_any_mut().downcast_mut().expect("expected language server")
}
