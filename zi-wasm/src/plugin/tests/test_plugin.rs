use super::*;

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn smoke() -> anyhow::Result<()> {
    let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
    let client = editor.client();

    let engine = engine();

    let mut store = new_store(engine, HostState::new(editor.client()));

    let local = LocalSet::new();
    local.spawn_local(editor.test_run(tasks));

    local
        .run_until(async move {
            let plugins = load(engine, &mut store, &["test"]).await?;

            for plugin in &plugins[..] {
                let dep = plugin.zi_api_dependency();
                assert_eq!(dep.call_get_name(&mut store).await?, "test");
                assert_eq!(dep.call_dependencies(&mut store).await?, Vec::<String>::new());

                let lifecycle = plugin.zi_api_lifecycle();
                let init = lifecycle.call_initialize(&mut store).await?;

                let rust = client
                    .with(|editor| {
                        let config = editor.language_config();
                        assert!(config.has_language_service(LanguageServiceId::new("test-ls")));
                        config.languages[&FileType::from_name("rust")].language_services.to_vec()
                    })
                    .await;
                assert_eq!(rust, [LanguageServiceId::new("test-ls")]);

                use crate::wit::exports::zi::api::command::{Arity, Command, CommandFlags};
                assert_eq!(
                    init,
                    InitializeResult {
                        commands: vec![
                            Command {
                                name: "foo".into(),
                                arity: Arity { min: 0, max: 1 },
                                opts: CommandFlags::RANGE
                            },
                            Command {
                                name: "stale-view".into(),
                                arity: Arity { min: 0, max: 0 },
                                opts: CommandFlags::empty()
                            },
                            Command {
                                name: "scratch".into(),
                                arity: Arity { min: 0, max: 0 },
                                opts: CommandFlags::empty()
                            },
                        ]
                    }
                );
                let handler = plugin.zi_api_command().handler();
                let handler_resource = handler.call_constructor(&mut store).await?;
                handler
                    .call_exec(&mut store, handler_resource, "foo", &["a"], false)
                    .await?
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                handler
                    .call_exec(&mut store, handler_resource, "scratch", &[], false)
                    .await?
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                handler_resource.resource_drop_async(&mut store).await?;

                lifecycle.call_shutdown(&mut store).await?;
            }

            Ok::<_, anyhow::Error>(())
        })
        .await
}

#[tokio::test]
#[cfg_attr(test, mutants::skip)]
async fn using_a_closed_view_traps_the_call() -> anyhow::Result<()> {
    let (editor, tasks) = Editor::new(crate::WasmBackend::default(), zi::Size::new(80, 24));
    let client = editor.client();
    let engine = engine();
    let mut store = new_store(engine, HostState::new(editor.client()));

    let local = LocalSet::new();
    local.spawn_local(editor.test_run(tasks));

    local
        .run_until(async move {
            let plugins = load(engine, &mut store, &["test"]).await?;
            let plugin = &plugins[0];
            plugin.zi_api_lifecycle().call_initialize(&mut store).await?;

            let handler = plugin.zi_api_command().handler();
            let handler_resource = handler.call_constructor(&mut store).await?;
            let err = handler
                .call_exec(&mut store, handler_resource, "stale-view", &[], false)
                .await
                .expect_err("using a closed view should trap");
            assert!(format!("{err:?}").contains("no longer exists"), "{err:?}");

            assert_eq!(client.with(|editor| editor.views().count()).await, 1);
            Ok::<_, anyhow::Error>(())
        })
        .await
}
