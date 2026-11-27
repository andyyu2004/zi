use std::future::Future;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{Stream, StreamExt, TryStreamExt};
use parking_lot::RwLock;
use slotmap::SlotMap;
use smol_str::SmolStr;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinSet;
use tokio_stream::wrappers::ReadDirStream;
use wasmtime::Engine;
use wasmtime::component::Component;
use zi::command::{CommandRange, Word};
use zi::{Client, dirs};

use super::host::PluginHost;
use super::runtime::{EpochTicker, engine, linker, new_store};
use super::state::HostState;
use crate::wit::Plugin;

/// The plugin manager responsible for loading and running wasm plugins and keeping track of their state.
/// This also provides the interface for the editor to interact with the plugins.
pub struct PluginManager {
    plugins: RwLock<SlotMap<PluginId, PluginState>>,
    initialized: watch::Sender<bool>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self { plugins: Default::default(), initialized: watch::Sender::new(false) }
    }
}

impl PluginManager {
    pub fn initialized(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut initialized = self.initialized.subscribe();
        async move {
            let _ = initialized.wait_for(|&initialized| initialized).await;
        }
    }

    #[must_use]
    pub(super) fn add(&self, _name: impl Into<SmolStr>, client: PluginClient) -> PluginId {
        // TODO name uniqueness check?
        self.plugins.write().insert(PluginState { client })
    }

    fn with_plugin<F, T>(&self, id: PluginId, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&PluginState) -> T,
    {
        self.plugins
            .read()
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("plugin not found: {id:?}"))
            .map(f)
    }

    fn plugin_client(&self, id: PluginId) -> anyhow::Result<PluginClient> {
        self.with_plugin(id, |state| state.client.clone())
    }

    pub async fn execute(
        &self,
        id: PluginId,
        name: Word,
        range: Option<CommandRange>,
        args: Box<[Word]>,
        force: bool,
    ) -> anyhow::Result<()> {
        let client = self.plugin_client(id)?;
        client.execute(name, range, args, force).await?;
        Ok(())
    }

    async fn instantiate_all(self: &Arc<Self>, client: &Client) -> Vec<PluginHost> {
        let engine = engine();
        let hosts = load_plugin_components(engine)
            .then(|component| self.instantiate(engine, client, component))
            .filter_map(|host| async {
                host.inspect_err(|err| {
                    tracing::error!(error = &**err, "error loading plugin (skipping)");
                })
                .ok()
            })
            .collect::<Vec<_>>()
            .await;
        tracing::info!(n = hosts.len(), "instantiated plugins");
        hosts
    }

    async fn instantiate(
        self: &Arc<Self>,
        engine: &'static Engine,
        client: &Client,
        component: anyhow::Result<Component>,
    ) -> anyhow::Result<PluginHost> {
        let component = component?;
        let linker = linker(engine)?;
        let mut store = new_store(engine, HostState::new(client.clone()));
        let plugin = Plugin::instantiate_async(&mut store, &component, &linker).await?;
        Ok(PluginHost::new(self.clone(), store, plugin))
    }
}

slotmap::new_key_type! {
    pub struct PluginId;
}

#[derive(Clone)]
pub(super) struct PluginClient(Sender<PluginRequest>);

impl PluginClient {
    pub(super) fn new(sender: Sender<PluginRequest>) -> Self {
        Self(sender)
    }

    pub(super) async fn execute(
        &self,
        name: Word,
        range: Option<CommandRange>,
        args: Box<[Word]>,
        force: bool,
    ) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.0.send(PluginRequest::ExecuteCommand { name, range, args, force, tx }).await?;
        rx.await?
    }
}

struct PluginState {
    client: PluginClient,
}

type Responder<T> = oneshot::Sender<anyhow::Result<T>>;

pub(super) enum PluginRequest {
    ExecuteCommand {
        name: Word,
        range: Option<CommandRange>,
        args: Box<[Word]>,
        force: bool,
        tx: Responder<()>,
    },
}

#[async_trait::async_trait]
impl zi::plugin::PluginManager for PluginManager {
    fn name(&self) -> &'static str {
        "wasm"
    }

    async fn start(self: Arc<Self>, client: Client) -> anyhow::Result<()> {
        let hosts = self.instantiate_all(&client).await;
        let timeout = client.with(|editor| *editor.settings().plugin_init_timeout.read()).await;
        let running = run_all(initialize_all(hosts, timeout).await);
        self.initialized.send_replace(true);
        wait_for_all(running).await;
        Ok(())
    }
}

fn load_plugin_components(
    engine: &'static Engine,
) -> impl Stream<Item = anyhow::Result<Component>> {
    let dirs = futures_util::stream::iter(dirs::plugin());
    let entries = dirs
        .then(|dir| async move {
            let entries = tokio::fs::read_dir(dir).await?;
            Ok::<_, io::Error>(ReadDirStream::new(entries))
        })
        .try_flatten();

    entries.map_err(Into::into).try_filter_map(move |entry| async move {
        if !entry.file_type().await?.is_file() {
            return Ok(None);
        }

        let component = Component::from_file(engine, entry.path())?;
        Ok(Some(component))
    })
}

pub(super) async fn initialize_all(
    hosts: Vec<PluginHost>,
    timeout: Duration,
) -> Vec<(PluginHost, mpsc::Receiver<PluginRequest>)> {
    let _ticker = EpochTicker::start(engine());
    let results =
        futures_util::future::join_all(hosts.into_iter().map(|host| initialize(host, timeout)))
            .await;
    results.into_iter().flatten().collect()
}

async fn initialize(
    mut host: PluginHost,
    timeout: Duration,
) -> Option<(PluginHost, mpsc::Receiver<PluginRequest>)> {
    let start = Instant::now();
    let result = tokio::time::timeout(timeout, host.initialize()).await;
    let name = host.name().unwrap_or("<unnamed>").to_owned();
    match result {
        Ok(Ok(receiver)) => {
            tracing::debug!(plugin = name, elapsed = ?start.elapsed(), "initialized plugin");
            return Some((host, receiver));
        }
        Ok(Err(err)) => tracing::error!(
            plugin = name,
            error = &*err,
            "plugin failed to initialize, skipping it (what it registered stays registered)"
        ),
        Err(_) => tracing::error!(
            plugin = name,
            ?timeout,
            "plugin took too long to initialize, skipping it (what it registered stays registered)"
        ),
    }
    host.abandon();
    None
}

fn run_all(hosts: Vec<(PluginHost, mpsc::Receiver<PluginRequest>)>) -> JoinSet<anyhow::Result<()>> {
    let mut running = JoinSet::new();
    for (host, receiver) in hosts {
        running.spawn(host.run(receiver));
    }
    running
}

async fn wait_for_all(mut running: JoinSet<anyhow::Result<()>>) {
    while let Some(res) = running.join_next().await {
        match res {
            Ok(Ok(())) => {}
            Ok(Err(err)) => tracing::error!(error = &*err, "error running plugin"),
            Err(err) => {
                tracing::error!(error = &err as &dyn std::error::Error, "error joining plugin");
            }
        }
    }
}
