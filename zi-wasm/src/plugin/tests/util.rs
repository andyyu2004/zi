use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::task::LocalSet;
use wasmtime::component::Component;
use zi::{Editor, LanguageServiceId};

use super::super::runtime::{Store, engine, linker, new_store};
use super::super::state::HostState;
use crate::test_util::build_plugin;
use crate::wit::Plugin;
use crate::wit::exports::zi::api::command::{Arity, Command, CommandError, CommandFlags};

/// Run `test` against a new editor whose event loop runs alongside it.
pub(super) async fn with_editor(
    test: impl AsyncFnOnce(zi::Client) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
    let client = editor.client();
    let local = LocalSet::new();
    local.spawn_local(editor.test_run(tasks));
    local.run_until(test(client)).await
}

pub(super) fn store(client: &zi::Client) -> Store {
    new_store(engine(), HostState::new(client.clone()))
}

pub(super) fn compile(wasm: &[u8]) -> anyhow::Result<Component> {
    Ok(Component::new(engine(), wasm)?)
}

pub(super) async fn instantiate_component(
    store: &mut Store,
    component: &Component,
) -> anyhow::Result<Plugin> {
    Ok(Plugin::instantiate_async(store, component, &linker(engine())?).await?)
}

pub(super) async fn instantiate(store: &mut Store, wasm: &[u8]) -> anyhow::Result<Plugin> {
    instantiate_component(store, &compile(wasm)?).await
}

/// Build and instantiate the bundled plugin in `plugins/<name>`.
pub(super) async fn load_bundled(store: &mut Store, name: &str) -> anyhow::Result<Plugin> {
    instantiate(store, &build_plugin(name)?).await
}

/// Run `cmd` through a new command handler. The outer error is a trap.
pub(super) async fn exec(
    store: &mut Store,
    plugin: &Plugin,
    cmd: &str,
    args: &[&str],
) -> wasmtime::Result<Result<(), CommandError>> {
    let handler = plugin.zi_api_command().handler();
    let resource = handler.call_constructor(&mut *store).await?;
    let res = handler.call_exec(&mut *store, resource, cmd, args, false).await?;
    resource.resource_drop_async(&mut *store).await?;
    Ok(res)
}

/// Run `cmd` through a new command handler and fail on a trap or a command error.
pub(super) async fn exec_ok(
    store: &mut Store,
    plugin: &Plugin,
    cmd: &str,
    args: &[&str],
) -> anyhow::Result<()> {
    exec(store, plugin, cmd, args).await?.map_err(|err| anyhow::anyhow!("`{cmd}` failed: {err}"))
}

pub(super) fn command(
    name: &str,
    arity: std::ops::RangeInclusive<u8>,
    opts: CommandFlags,
) -> Command {
    let (min, max) = arity.into_inner();
    Command { name: name.into(), arity: Arity { min, max }, opts }
}

/// The text of the active buffer, and whether it is readonly.
pub(super) async fn active_buffer(client: &zi::Client) -> (String, bool) {
    client
        .with(|editor| {
            let buf = editor.buffer(zi::Active);
            (buf.text().to_string(), *buf.settings().readonly.read())
        })
        .await
}

/// Open `path`, waiting for it to load.
pub(super) async fn open(
    client: &zi::Client,
    path: impl Into<PathBuf>,
    flags: zi::OpenFlags,
) -> zi::Result<zi::BufferId> {
    let path = path.into();
    client.with(move |editor| editor.open(path, flags)).await?.await
}

pub(super) fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// A Go file in the zi-lsp test module, for starting gopls.
pub(super) fn go_test_file() -> PathBuf {
    manifest_dir().join("../zi-lsp/tests/zi-lsp/testdirs/lsp-test/main.go")
}

/// Resolve once `service_id` has initialized, or fail after 40 seconds.
pub(super) fn on_initialized(
    service_id: LanguageServiceId,
) -> impl Future<Output = anyhow::Result<()>> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    zi::event::subscribe_with::<zi::event::DidInitializeLanguageService>(move |_editor, event| {
        if event.service_id == service_id {
            let _ = tx.send(());
        }
        zi::event::HandlerResult::Continue
    });
    async move {
        tokio::time::timeout(Duration::from_secs(40), rx.recv())
            .await
            .map_err(|_| anyhow::anyhow!("`{service_id}` did not initialize"))?;
        Ok(())
    }
}
