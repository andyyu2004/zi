//! The `:lsp` report in `plugins/lspinfo`.

use expect_test::expect;
use zi::LanguageConfig;
use zi_lsp::LanguageServerConfig;

use super::super::runtime::Store;
use super::util::*;
use crate::wit::Plugin;

/// Run `:lsp` and return the report it opened, with the working directory replaced by `$CWD`.
async fn lsp_report(
    store: &mut Store,
    plugin: &Plugin,
    client: &zi::Client,
) -> anyhow::Result<String> {
    exec_ok(store, plugin, "lsp", &[]).await?;
    let (text, readonly) = active_buffer(client).await;
    assert!(readonly, "the report should be in a readonly buffer");
    Ok(text.replace(&*std::env::current_dir()?.to_string_lossy(), "$CWD"))
}

/// gopls for Go, a server whose executable doesn't exist for TOML, and an undefined one for JSON.
async fn configure_services(client: &zi::Client) {
    client
        .with(|editor| {
            editor
                .language_config_mut()
                .add_language_service("gopls", LanguageServerConfig::new("gopls", []))
                .add_language_service(
                    "missing-ls",
                    LanguageServerConfig::new("zi-no-such-server", ["--stdio".into()]),
                )
                .add_language(zi::filetype!(go), LanguageConfig::new(["gopls".into()]))
                .add_language(zi::filetype!(toml), LanguageConfig::new(["missing-ls".into()]))
                .add_language(zi::filetype!(json), LanguageConfig::new(["undefined-ls".into()]));
        })
        .await;
}

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn report_before_any_service_starts() -> anyhow::Result<()> {
    with_editor(async |client| {
        configure_services(&client).await;
        let mut store = store(&client);
        let plugin = load_bundled(&mut store, "lspinfo").await?;
        assert_eq!(plugin.zi_api_dependency().call_get_name(&mut store).await?, "lspinfo");
        plugin.zi_api_lifecycle().call_initialize(&mut store).await?;

        expect![[r"
            gopls: not started
              command: gopls
              filetypes: go

            missing-ls: not started
              command: zi-no-such-server --stdio
              filetypes: toml

            undefined-ls: not defined
              filetypes: json

            Language server stderr is logged at debug level under the `zi_lsp::stderr` target.
            Enable it with ZI_LOG=zi_lsp=debug.
        "]]
        .assert_eq(&lsp_report(&mut store, &plugin, &client).await?);
        assert_eq!(client.with(|editor| editor.views().count()).await, 2);
        Ok(())
    })
    .await
}

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn report_running_and_failed_services() -> anyhow::Result<()> {
    with_editor(async |client| {
        configure_services(&client).await;
        let mut store = store(&client);
        let plugin = load_bundled(&mut store, "lspinfo").await?;
        plugin.zi_api_lifecycle().call_initialize(&mut store).await?;

        let initialized = on_initialized("gopls".into());
        open(&client, go_test_file(), zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
        initialized.await?;

        let flags = zi::OpenFlags::SPAWN_LANGUAGE_SERVICES | zi::OpenFlags::READONLY;
        let res = open(&client, manifest_dir().join("Cargo.toml"), flags).await;
        assert!(res.is_err(), "starting a missing executable should fail");

        let report = lsp_report(&mut store, &plugin, &client).await?;
        let lines = report.lines().collect::<Vec<_>>();
        assert_eq!(lines[..3], ["gopls: running", "  command: gopls", "  filetypes: go"]);
        assert!(lines[3].starts_with("  server: gopls"), "{report}");
        assert!(lines[4].starts_with("  pid: "), "{report}");
        assert_eq!(lines[5], "  root: $CWD", "{report}");
        assert_eq!(lines[6], "  position encoding: utf-16", "{report}");
        assert!(lines[7].starts_with("  capabilities: definition, "), "{report}");
        assert_eq!(lines[8], "  buffers:", "{report}");
        assert!(lines[9].ends_with("/lsp-test/main.go"), "{report}");
        assert_eq!(
            lines[11..14],
            [
                "missing-ls: failed to start",
                "  command: zi-no-such-server --stdio",
                "  error: No such file or directory (os error 2)",
            ],
            "{report}"
        );
        Ok(())
    })
    .await
}
