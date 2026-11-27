use super::*;

async fn run_lsp_command(
    store: &mut Store,
    plugin: &Plugin,
    client: &zi::Client,
) -> anyhow::Result<String> {
    let handler = plugin.zi_api_command().handler();
    let resource = handler.call_constructor(&mut *store).await?;
    handler
        .call_exec(&mut *store, resource, "lsp", &[], false)
        .await?
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    resource.resource_drop_async(&mut *store).await?;

    let (readonly, text) = client
        .with(|editor| {
            let buf = editor.buffer(zi::Active);
            (*buf.settings().readonly.read(), buf.text().to_string())
        })
        .await;
    assert!(readonly, "the report should be in a readonly buffer");
    Ok(text.replace(&*std::env::current_dir()?.to_string_lossy(), "$CWD"))
}

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn lspinfo_plugin() -> anyhow::Result<()> {
    use expect_test::expect;
    use zi::LanguageConfig;
    use zi_lsp::LanguageServerConfig;

    let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
    let client = editor.client();
    let engine = engine();
    let mut store = new_store(engine, HostState::new(editor.client()));

    let local = LocalSet::new();
    local.spawn_local(editor.test_run(tasks));

    local
        .run_until(async move {
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
                        .add_language(
                            zi::filetype!(toml),
                            LanguageConfig::new(["missing-ls".into()]),
                        )
                        .add_language(
                            zi::filetype!(json),
                            LanguageConfig::new(["undefined-ls".into()]),
                        );
                })
                .await;

            let plugins = load(engine, &mut store, &["lspinfo"]).await?;
            let plugin = &plugins[0];
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
            .assert_eq(&run_lsp_command(&mut store, plugin, &client).await?);
            assert_eq!(client.with(|editor| editor.views().count()).await, 2);

            let (tx, mut initialized) = tokio::sync::mpsc::unbounded_channel();
            zi::event::subscribe_with::<zi::event::DidInitializeLanguageService>(
                move |_editor, _event| {
                    let _ = tx.send(());
                    zi::event::HandlerResult::Continue
                },
            );

            let go = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../zi-lsp/tests/zi-lsp/testdirs/lsp-test/main.go");
            client
                .with(move |editor| editor.open(go, zi::OpenFlags::SPAWN_LANGUAGE_SERVICES))
                .await?
                .await?;
            tokio::time::timeout(Duration::from_secs(40), initialized.recv())
                .await
                .map_err(|_| anyhow::anyhow!("gopls did not initialize"))?;

            let toml = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
            let res = client
                .with(move |editor| {
                    editor.open(
                        toml,
                        zi::OpenFlags::SPAWN_LANGUAGE_SERVICES | zi::OpenFlags::READONLY,
                    )
                })
                .await?
                .await;
            assert!(res.is_err(), "starting a missing executable should fail");

            let report = run_lsp_command(&mut store, plugin, &client).await?;
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
            Ok::<_, anyhow::Error>(())
        })
        .await
}
