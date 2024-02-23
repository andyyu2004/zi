use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::process::Stdio;

use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router::Router;
use async_lsp::tracing::TracingLayer;
use async_lsp::{LanguageClient, Result, ServerSocket};
use tower::ServiceBuilder;

pub struct Server {
    // Storing child with `kill_on_drop` set so that it gets killed when this struct is dropped
    #[allow(dead_code)]
    child: async_process::Child,
    server: ServerSocket,
}

impl Server {
    pub fn start<C: LanguageClient + Send + 'static>(
        client: C,
        root: &Path,
        cmd: &Path,
    ) -> Result<Server> {
        let (main_loop, server) = async_lsp::MainLoop::new_client(|_server| {
            ServiceBuilder::new()
                .layer(TracingLayer::default())
                .layer(CatchUnwindLayer::default())
                .layer(ConcurrencyLayer::default())
                .service(Router::new(client))
        });

        let mut child = async_process::Command::new(cmd)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take().unwrap();

        let _handle = tokio::spawn(async move {
            main_loop.run_buffered(stdout, stdin).await.expect("main loop failed");
        });

        Ok(Server { child, server })
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

