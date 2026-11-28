//! The API test plugin in `plugins/test`. It asserts on the API itself as it runs.

use zi::{FileType, LanguageServiceId};

use super::util::*;
use crate::wit::exports::zi::api::command::CommandFlags;
use crate::wit::exports::zi::api::lifecycle::InitializeResult;

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn smoke() -> anyhow::Result<()> {
    with_editor(async |client| {
        let mut store = store(&client);
        let plugin = load_bundled(&mut store, "test").await?;

        let dep = plugin.zi_api_dependency();
        assert_eq!(dep.call_get_name(&mut store).await?, "test");
        assert_eq!(dep.call_dependencies(&mut store).await?, Vec::<String>::new());

        let lifecycle = plugin.zi_api_lifecycle();
        assert_eq!(
            lifecycle.call_initialize(&mut store).await?,
            InitializeResult {
                commands: vec![
                    command("foo", 0..=1, CommandFlags::RANGE),
                    command("stale-view", 0..=0, CommandFlags::empty()),
                    command("scratch", 0..=0, CommandFlags::empty()),
                ]
            }
        );

        let rust = client
            .with(|editor| {
                let config = editor.language_config();
                assert!(config.has_language_service(LanguageServiceId::new("test-ls")));
                config.languages[&FileType::from_name("rust")].language_services.to_vec()
            })
            .await;
        assert_eq!(rust, [LanguageServiceId::new("test-ls")]);

        exec_ok(&mut store, &plugin, "foo", &["a"]).await?;
        exec_ok(&mut store, &plugin, "scratch", &[]).await?;

        lifecycle.call_shutdown(&mut store).await?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn using_a_closed_view_traps_the_call() -> anyhow::Result<()> {
    with_editor(async |client| {
        let mut store = store(&client);
        let plugin = load_bundled(&mut store, "test").await?;
        plugin.zi_api_lifecycle().call_initialize(&mut store).await?;

        let err = exec(&mut store, &plugin, "stale-view", &[])
            .await
            .expect_err("using a closed view should trap");
        assert!(format!("{err:?}").contains("no longer exists"), "{err:?}");

        assert_eq!(client.with(|editor| editor.views().count()).await, 1);
        Ok(())
    })
    .await
}
