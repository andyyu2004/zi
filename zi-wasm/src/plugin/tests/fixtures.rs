//! The TypeScript, JavaScript and Rust fixture plugins, built through `zi-plugin`.

use zi::{FileType, LanguageServiceId};
use zi_plugin::Language;

use super::util::*;
use crate::test_util::build_fixture_plugin;
use crate::wit::exports::zi::api::command::{CommandError, CommandFlags};
use crate::wit::exports::zi::api::lifecycle::InitializeResult;

async fn check_fixture_plugin(lang: Language, name: &str) -> anyhow::Result<()> {
    with_editor(async |client| {
        let mut store = store(&client);
        let plugin = instantiate(&mut store, &build_fixture_plugin(lang)?).await?;
        assert_eq!(plugin.zi_api_dependency().call_get_name(&mut store).await?, name);

        assert_eq!(
            plugin.zi_api_lifecycle().call_initialize(&mut store).await?,
            InitializeResult {
                commands: vec![command("insert-mode", 0..=0, CommandFlags::empty())]
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

        exec_ok(&mut store, &plugin, "insert-mode", &[]).await?;
        assert_eq!(client.with(|editor| editor.mode()).await, zi::Mode::Insert);

        assert_eq!(
            exec(&mut store, &plugin, "nope", &[]).await?,
            Err(CommandError::Message("unexpected command: `nope`".into()))
        );
        Ok(())
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
