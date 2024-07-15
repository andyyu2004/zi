mod adaptor;

use std::ffi::OsStr;
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
use tokio::io::AsyncWriteExt;
use tokio_util::compat::FuturesAsyncReadCompatExt as _;
use tower::ServiceBuilder;

pub use self::adaptor::ToLanguageService;

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
