use std::sync::Arc;

use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use tokio::select;
use tokio::sync::mpsc;
use wasmtime::component::ResourceAny;
use zi::Client;
use zi::command::{CommandRange, Handler, Word};

use super::manager::{PluginClient, PluginManager, PluginRequest};
use super::runtime::Store;
use crate::wit::Plugin;
use crate::wit::exports::zi::api::lifecycle::Command;

pub(super) struct PluginHost {
    manager: Arc<PluginManager>,
    store: Store,
    plugin: Plugin,
    handler: Option<ResourceAny>,
    name: Option<String>,
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        if let Some(handler) = self.handler.take() {
            tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(async move {
                    let _ = handler.resource_drop_async(&mut self.store).await;
                });
            });
        }
    }
}

impl PluginHost {
    pub(super) fn new(manager: Arc<PluginManager>, store: Store, plugin: Plugin) -> Self {
        Self { manager, store, plugin, handler: None, name: None }
    }

    pub(super) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub(super) fn abandon(&mut self) {
        self.handler.take();
    }

    pub(super) async fn initialize(&mut self) -> anyhow::Result<mpsc::Receiver<PluginRequest>> {
        let name = self.plugin.zi_api_dependency().call_get_name(&mut self.store).await?;
        self.name = Some(name.clone());

        let init = self.plugin.zi_api_lifecycle().call_initialize(&mut self.store).await?;
        let handler = self.plugin.zi_api_command().handler();
        self.handler = Some(handler.call_constructor(&mut self.store).await?);

        let (sender, receiver) = mpsc::channel(16);
        let client = PluginClient::new(sender);
        let editor = self.store.data().client().clone();
        register_commands(&editor, &client, init.commands).await;
        let _id = self.manager.add(name, client);
        Ok(receiver)
    }

    pub(super) async fn run(
        mut self,
        mut receiver: mpsc::Receiver<PluginRequest>,
    ) -> anyhow::Result<()> {
        loop {
            select! {
                msg = receiver.recv() => match msg {
                    Some(req) => self.handle_request(req).await?,
                    None => break,
                }
            }
        }
        self.shutdown().await
    }

    async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.plugin.zi_api_lifecycle().call_shutdown(&mut self.store).await?;
        if let Some(handler) = self.handler.take() {
            handler.resource_drop_async(&mut self.store).await?;
        }
        Ok(())
    }

    async fn handle_request(&mut self, req: PluginRequest) -> anyhow::Result<()> {
        match req {
            PluginRequest::ExecuteCommand { name, range, args, tx, force } => {
                let _ = range;
                let result = self.exec(&name, &args, force).await?;
                let _ = tx.send(result.map_err(|e| anyhow::anyhow!("{e}")));
            }
        }
        Ok(())
    }

    async fn exec(
        &mut self,
        name: &str,
        args: &[Word],
        force: bool,
    ) -> wasmtime::Result<Result<(), crate::wit::exports::zi::api::command::CommandError>> {
        let handler = self.handler.expect("handler not initialized");
        let args = args.iter().map(|s| s.as_ref()).collect::<Box<_>>();
        self.plugin
            .zi_api_command()
            .handler()
            .call_exec(&mut self.store, handler, name, &args, force)
            .await
    }
}

async fn register_commands(editor: &Client, client: &PluginClient, commands: Vec<Command>) {
    for cmd in commands {
        let Ok(name) = Word::try_from(cmd.name.as_str()) else {
            tracing::error!(command = cmd.name, "invalid command name");
            continue;
        };
        let handler = Handler::new(
            name.clone(),
            cmd.arity.into(),
            cmd.opts.into(),
            PluginExecutor { name, client: client.clone() },
        );
        editor.with(move |editor| editor.register_command(handler)).await;
    }
}

struct PluginExecutor {
    name: Word,
    client: PluginClient,
}

impl zi::command::Executor for PluginExecutor {
    fn execute(
        &self,
        _client: Client,
        range: Option<CommandRange>,
        args: Box<[Word]>,
        force: bool,
    ) -> BoxFuture<'static, Result<(), zi::Error>> {
        let client = self.client.clone();
        let name = self.name.clone();
        async move { client.execute(name, range, args, force).await }.boxed()
    }
}
