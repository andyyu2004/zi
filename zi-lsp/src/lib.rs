mod client;
mod server;

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
    lsp_types, Error, ErrorCode, LanguageClient, LanguageServer, ResponseError, Result,
    ServerSocket,
};
use futures_util::future::BoxFuture;
use futures_util::TryFutureExt;
use tokio::io::AsyncWriteExt;
use tokio_util::compat::FuturesAsyncReadCompatExt as _;
use tower::ServiceBuilder;
use zi_language_service::{LanguageService, LanguageServiceConfig};

use self::client::ToLanguageClient;
pub use self::server::ToLanguageService;

pub fn start<C>(
    client: C,
    cwd: impl AsRef<Path>,
    cmd: impl AsRef<OsStr>,
    args: impl IntoIterator<Item = impl AsRef<OsStr>>,
) -> Result<(ServerSocket, impl Future<Output = Result<()>> + 'static)>
where
    C: LanguageClient<NotifyResult = ControlFlow<crate::Result<()>>, Error = ResponseError>
        + Send
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

impl<E: Send + 'static> LanguageServiceConfig<E> for LanguageServerConfig {
    fn spawn(
        &self,
        cwd: &Path,
        client: Box<dyn zi_language_service::LanguageClient<E>>,
    ) -> anyhow::Result<(Box<dyn LanguageService<E> + Send>, BoxFuture<'static, anyhow::Result<()>>)>
    {
        tracing::debug!(command = ?self.command, args = ?self.args, "spawn language server");
        let (server, fut) =
            start(ToLanguageClient::new(client), cwd, &self.command, &self.args[..])?;
        Ok((Box::new(ToLanguageService::new(server)), Box::pin(fut.map_err(Into::into))))
    }
}
