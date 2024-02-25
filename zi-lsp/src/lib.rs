use std::ffi::OsStr;
use std::ops::{ControlFlow, Deref, DerefMut};
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
use tower::ServiceBuilder;

pub struct Server {
    // Storing child with `kill_on_drop` set so that it gets killed when this struct is dropped
    #[allow(dead_code)]
    child: async_process::Child,
    server: ServerSocket,
    handle: tokio::task::JoinHandle<()>,
}

impl Server {
    pub async fn shutdown(mut self) -> crate::Result<()> {
        self.server.shutdown(()).await?;
        self.server.exit(())?;
        self.handle.await.expect("server task failed");
        Ok(())
    }

    pub fn start<C>(
        client: C,
        cwd: impl AsRef<Path>,
        cmd: impl AsRef<OsStr>,
        args: impl IntoIterator<Item = impl AsRef<OsStr>>,
    ) -> Result<Server>
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

        let mut child = async_process::Command::new(cmd)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take().unwrap();

        let handle = tokio::spawn(async move {
            let _ = main_loop.run_buffered(stdout, stdin).await;
        });

        Ok(Server { child, server, handle })
    }
}

impl Deref for Server {
    type Target = ServerSocket;

    fn deref(&self) -> &Self::Target {
        &self.server
    }
}

impl DerefMut for Server {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.server
    }
}
