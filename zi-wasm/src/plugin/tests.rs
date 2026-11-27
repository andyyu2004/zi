use tokio::task::LocalSet;
use zi::Editor;

use std::sync::Arc;
use std::time::{Duration, Instant};

use wasmtime::Engine;
use wasmtime::component::Component;
use zi::{FileType, LanguageServiceId};
use zi_plugin::Language;

use super::host::PluginHost;
use super::manager::{PluginClient, PluginManager, initialize_all};
use super::runtime::{Store, engine, linker, new_store};
use super::state::HostState;
use crate::wit::Plugin;

use crate::test_util::{build_fixture_plugin, build_plugin, build_source_plugin};
use crate::wit::exports::zi::api::lifecycle::InitializeResult;

mod lspinfo;
mod test_plugin;

async fn load(engine: &Engine, store: &mut Store, names: &[&str]) -> anyhow::Result<Box<[Plugin]>> {
    let mut plugins = Vec::with_capacity(names.len());
    for name in names {
        let component = Component::new(engine, build_plugin(name)?)?;
        let linker = linker(engine)?;
        let plugin = Plugin::instantiate_async(&mut *store, &component, &linker).await?;
        plugins.push(plugin);
    }

    Ok(plugins.into_boxed_slice())
}

async fn check_fixture_plugin(lang: Language, name: &str) -> anyhow::Result<()> {
    let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
    let client = editor.client();
    let engine = engine();
    let mut store = new_store(engine, HostState::new(editor.client()));

    let local = LocalSet::new();
    local.spawn_local(editor.test_run(tasks));

    local
        .run_until(async move {
            let component = Component::new(engine, build_fixture_plugin(lang)?)?;
            let plugin =
                Plugin::instantiate_async(&mut store, &component, &linker(engine)?).await?;
            assert_eq!(plugin.zi_api_dependency().call_get_name(&mut store).await?, name);

            use crate::wit::exports::zi::api::command::{
                Arity, Command, CommandError, CommandFlags,
            };
            let init = plugin.zi_api_lifecycle().call_initialize(&mut store).await?;
            assert_eq!(
                init,
                InitializeResult {
                    commands: vec![Command {
                        name: "insert-mode".into(),
                        arity: Arity { min: 0, max: 0 },
                        opts: CommandFlags::empty(),
                    }]
                }
            );

            let servers = client
                .with(|editor| {
                    let config = editor.language_config();
                    assert!(config.has_language_service(LanguageServiceId::new("test-ls")));
                    config.languages[&FileType::from_name("typescript")].language_services.to_vec()
                })
                .await;
            assert_eq!(servers, [LanguageServiceId::new("test-ls")]);

            let handler = plugin.zi_api_command().handler();
            let resource = handler.call_constructor(&mut store).await?;
            handler
                .call_exec(&mut store, resource, "insert-mode", &[], false)
                .await?
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            assert_eq!(client.with(|editor| editor.mode()).await, zi::Mode::Insert);

            let err = handler.call_exec(&mut store, resource, "nope", &[], false).await?;
            assert_eq!(err, Err(CommandError::Message("unexpected command: `nope`".into())));
            resource.resource_drop_async(&mut store).await?;
            Ok::<_, anyhow::Error>(())
        })
        .await
}

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn typescript_plugin() -> anyhow::Result<()> {
    check_fixture_plugin(Language::TypeScript, "test-ts").await
}

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn javascript_plugin() -> anyhow::Result<()> {
    check_fixture_plugin(Language::JavaScript, "test-js").await
}

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn rust_plugin() -> anyhow::Result<()> {
    check_fixture_plugin(Language::Rust, "test-rs").await
}

const SPIN_IN_INITIALIZE: &str = r#"
export const dependency = { getName: () => "test-spin", dependencies: () => [] };
export const lifecycle = {
  initialize() {
    for (;;) {}
  },
  shutdown() {},
};
class Handler {
  exec() {}
}
export const command = { Handler };
"#;

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn plugins_initialize_concurrently_within_a_timeout() -> anyhow::Result<()> {
    let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
    let client = editor.client();
    let engine = engine();

    let local = LocalSet::new();
    local.spawn_local(editor.test_run(tasks));

    local
        .run_until(async move {
            let spin = Component::new(
                engine,
                build_source_plugin(Language::JavaScript, "test-spin", SPIN_IN_INITIALIZE)?,
            )?;
            let ok = Component::new(engine, build_fixture_plugin(Language::JavaScript)?)?;

            let manager = Arc::new(PluginManager::default());
            let linker = linker(engine)?;
            let mut hosts = vec![];
            for component in [&spin, &spin, &ok] {
                let mut store = new_store(engine, HostState::new(client.clone()));
                let plugin = Plugin::instantiate_async(&mut store, component, &linker).await?;
                hosts.push(PluginHost::new(manager.clone(), store, plugin));
            }

            let timeout = Duration::from_millis(500);
            let start = Instant::now();
            let initialized = initialize_all(hosts, timeout).await;
            let elapsed = start.elapsed();

            let names = initialized
                .into_iter()
                .map(|(mut host, _receiver)| {
                    host.abandon();
                    host.name().map(str::to_owned)
                })
                .collect::<Vec<_>>();
            assert_eq!(names, [Some("test-js".to_owned())]);
            assert!(elapsed >= timeout && elapsed < 2 * timeout, "took {elapsed:?}");
            assert!(
                client
                    .with(|editor| {
                        editor
                            .language_config()
                            .has_language_service(LanguageServiceId::new("test-ls"))
                    })
                    .await
            );
            Ok::<_, anyhow::Error>(())
        })
        .await
}

const SPIN_IN_COMMAND: &str = r#"
export const dependency = { getName: () => "test-spin-command", dependencies: () => [] };
export const lifecycle = {
  initialize() {
    return { commands: [] };
  },
  shutdown() {},
};
class Handler {
  exec(cmd) {
    if (cmd === "spin") {
      for (;;) {}
    }
  }
}
export const command = { Handler };
"#;

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn a_plugin_command_that_runs_too_long_stops_the_plugin() -> anyhow::Result<()> {
    let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
    let client = editor.client();
    let engine = engine();

    let local = LocalSet::new();
    local.spawn_local(editor.test_run(tasks));

    local
        .run_until(async move {
            let timeout = Duration::from_millis(300);
            client
                .with(move |editor| editor.settings().plugin_command_timeout.write(timeout))
                .await;

            let component = Component::new(
                engine,
                build_source_plugin(Language::JavaScript, "test-spin-command", SPIN_IN_COMMAND)?,
            )?;
            let mut store = new_store(engine, HostState::new(client.clone()));
            let plugin =
                Plugin::instantiate_async(&mut store, &component, &linker(engine)?).await?;
            let host = PluginHost::new(Arc::new(PluginManager::default()), store, plugin);
            let [(host, _receiver)] =
                <[_; 1]>::try_from(initialize_all(vec![host], Duration::from_secs(5)).await)
                    .map_err(|_| anyhow::anyhow!("plugin didn't initialize"))?;

            let word = |name: &'static str| zi::command::Word::try_from(name).unwrap();
            let (sender, receiver) = tokio::sync::mpsc::channel(1);
            let running = tokio::task::spawn_local(host.run(receiver));
            let plugin = PluginClient::new(sender);

            plugin.execute(word("ok"), None, Box::new([]), false).await?;

            let start = Instant::now();
            let err = plugin.execute(word("spin"), None, Box::new([]), false).await;
            let elapsed = start.elapsed();
            assert!(err.unwrap_err().to_string().contains("took longer than"));
            assert!(elapsed >= timeout && elapsed < 2 * timeout, "took {elapsed:?}");

            assert!(running.await?.is_err());
            assert!(plugin.execute(word("ok"), None, Box::new([]), false).await.is_err());
            Ok::<_, anyhow::Error>(())
        })
        .await
}
