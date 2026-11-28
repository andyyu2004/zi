//! Plugins that run too long while initializing or running a command.

use std::sync::Arc;
use std::time::{Duration, Instant};

use zi::LanguageServiceId;
use zi_plugin::Language;

use super::super::host::PluginHost;
use super::super::manager::{PluginClient, PluginManager, initialize_all};
use super::util::*;
use crate::test_util::{build_fixture_plugin, build_source_plugin};

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
    with_editor(async |client| {
        let spin =
            compile(&build_source_plugin(Language::JavaScript, "test-spin", SPIN_IN_INITIALIZE)?)?;
        let ok = compile(&build_fixture_plugin(Language::JavaScript)?)?;

        let manager = Arc::new(PluginManager::default());
        let mut hosts = vec![];
        for component in [&spin, &spin, &ok] {
            let mut store = store(&client);
            let plugin = instantiate_component(&mut store, component).await?;
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
                    editor.language_config().has_language_service(LanguageServiceId::new("test-ls"))
                })
                .await
        );
        Ok(())
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
    with_editor(async |client| {
        let timeout = Duration::from_millis(300);
        client.with(move |editor| editor.settings().plugin_command_timeout.write(timeout)).await;

        let wasm = build_source_plugin(Language::JavaScript, "test-spin-command", SPIN_IN_COMMAND)?;
        let mut store = store(&client);
        let plugin = instantiate(&mut store, &wasm).await?;
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
        Ok(())
    })
    .await
}
