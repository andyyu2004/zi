use rand::RngExt as _;
use stdx::bomb::DropBomb;
use tokio::sync::watch;
use zi::{TextBase, deltas, lstypes};

use super::*;

#[tokio::test]
async fn lsp_sync_multiple_servers() -> zi::Result<()> {
    fn server(
        opens: Arc<AtomicUsize>,
        changes: Arc<AtomicUsize>,
    ) -> FakeLanguageServerTemplate<()> {
        FakeLanguageServer::builder()
            .request::<request::Initialize, _>(|_, _| async {
                Ok(lsp_types::InitializeResult {
                    capabilities: lsp_types::ServerCapabilities {
                        position_encoding: Some(lsp_types::PositionEncodingKind::UTF8),
                        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
                            lsp_types::TextDocumentSyncKind::FULL,
                        )),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            })
            .notification::<notification::Initialized>(|_, _| Ok(()))
            .notification::<notification::DidOpenTextDocument>(move |_, _| {
                opens.fetch_add(1, atomic::Ordering::SeqCst);
                Ok(())
            })
            .notification::<notification::DidChangeTextDocument>(move |_, _| {
                changes.fetch_add(1, atomic::Ordering::SeqCst);
                Ok(())
            })
            .finish(())
    }

    let cx = new("").await;

    let counts = [(); 4].map(|_| Arc::new(AtomicUsize::new(0)));
    let [a_opens, a_changes, b_opens, b_changes] = counts.clone();
    cx.with(move |editor| {
        editor
            .language_config_mut()
            .add_language(
                zi::filetype!(text),
                zi::LanguageConfig::new(["server-a".into(), "server-b".into()]),
            )
            .add_language_service("server-a", server(a_opens, a_changes))
            .add_language_service("server-b", server(b_opens, b_changes));
    })
    .await;

    let buf = cx.open_tmp("abc\n", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
    while !cx
        .with(|editor| {
            editor.language_service("server-a".into()).is_some()
                && editor.language_service("server-b".into()).is_some()
        })
        .await
    {
        tokio::task::yield_now().await;
    }
    cx.with(move |editor| editor.edit(buf, &zi::Deltas::insert_at(0, "x"))).await.unwrap();

    let [a_opens, a_changes, b_opens, b_changes] = counts.map(|c| c.load(atomic::Ordering::SeqCst));
    assert_eq!((a_opens, b_opens), (1, 1), "didOpen count per server");
    assert_eq!((a_changes, b_changes), (1, 1), "didChange count per server");

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn lsp_change_no_sync() -> zi::Result<()> {
    let cx = new("").await;
    let bomb = DropBomb::new("initialize should be called");

    cx.setup_lang_server(zi::filetype!(text), "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(move |_, _| {
                bomb.defuse();
                async { Ok(lsp_types::InitializeResult::default()) }
            })
            .notification::<notification::DidChangeTextDocument>(move |_st, params| {
                panic!("should not call did_change as capability is not set: {:?}", params)
            })
    })
    .await;

    let buf = cx.open_tmp("", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
    cx.with(move |editor| editor.edit(buf, &zi::Deltas::insert_at(0, "abc"))).await.unwrap();

    cx.cleanup().await;

    Ok(())
}

#[tokio::test]
async fn lsp_change_full_sync() -> zi::Result<()> {
    let cx = new("").await;
    let bomb = DropBomb::new("did_change should be called");

    let expected_events = ExpectedSequence::new([
        vec![lsp_change_event!("abc\n")],
        vec![lsp_change_event!("dbc\n")],
        // should probably try and merge some of these events into 1
        vec![lsp_change_event!("abc\n")],
        vec![lsp_change_event!("abc")],
        vec![lsp_change_event!("abc\n")],
        vec![lsp_change_event!("dbc\n")],
    ]);

    cx.setup_lang_server(zi::filetype!(text), "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(|_, _| async {
                Ok(lsp_types::InitializeResult {
                    capabilities: lsp_types::ServerCapabilities {
                        position_encoding: Some(lsp_types::PositionEncodingKind::UTF8),
                        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
                            lsp_types::TextDocumentSyncKind::FULL,
                        )),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            })
            .notification::<notification::DidChangeTextDocument>(move |_st, params| {
                bomb.defuse();
                expected_events.assert_eq(&params.content_changes);
                Ok(())
            })
    })
    .await;

    let edits = vec![lsp_types::TextEdit {
        range: lsp_types::Range {
            start: lsp_types::Position { line: 0, character: 0 },
            end: lsp_types::Position { line: 0, character: 1 },
        },
        new_text: "d".to_string(),
    }];

    let text = "abc";
    let deltas = zi_lsp::from_proto::deltas(lstypes::PositionEncoding::Utf8, text, edits).unwrap();
    let buf = cx.open_tmp(text, zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
    cx.with(move |editor| editor.edit(buf, &deltas)).await.unwrap();

    assert!(cx.with(move |editor| editor.undo(buf)).await.unwrap());

    assert!(cx.with(move |editor| editor.redo(buf)).await.unwrap());

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn lsp_changes_incremental_utf8() -> zi::Result<()> {
    let cx = new("").await;

    // It may look like the events are out of order, but that's due to the way zi sorts deltas.
    // This is also important for LSP as edits are applied in order. Ordering this way avoids changes affecting each other.
    let expected_events = ExpectedSequence::new([
        vec![lsp_change_event!("\n")],
        vec![lsp_change_event!(0:0..0:0 => "abc")],
        vec![lsp_change_event!(0:3..0:3 => "de")],
        vec![lsp_change_event!(0:2..0:2 => "z"), lsp_change_event!(0:0..0:0 => "©")],
    ]);

    cx.setup_lang_server(zi::filetype!(text), "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(move |_, _params| async {
                Ok(lsp_types::InitializeResult {
                    capabilities: lsp_types::ServerCapabilities {
                        position_encoding: Some(lsp_types::PositionEncodingKind::UTF8),
                        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
                            lsp_types::TextDocumentSyncKind::INCREMENTAL,
                        )),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            })
            .notification::<notification::DidChangeTextDocument>(move |_, params| {
                expected_events.assert_eq(&params.content_changes);
                Ok(())
            })
    })
    .await;

    let buf = cx.open_tmp("", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;

    cx.with(move |editor| {
        editor.edit(buf, &zi::Deltas::insert_at(0, "abc"))?;
        assert_eq!(editor.text(buf), "abc\n");

        editor.edit(buf, &deltas![3..3 => "de"])?;
        assert_eq!(editor.text(buf), "abcde\n");

        editor.edit(buf, &deltas![0..0 => "©", 2..2 => "z"])?;
        assert_eq!(editor.text(buf), "©abzcde\n");

        Ok::<_, zi::EditError>(())
    })
    .await?;

    cx.cleanup().await;

    Ok(())
}

#[tokio::test]
async fn lsp_changes_incremental_utf16() -> zi::Result<()> {
    let cx = new("").await;

    // It may look like the events are out of order, but that's due to the way zi sorts deltas.
    // This is also important for LSP as edits are applied in order. Ordering this way avoids changes affecting each other.
    let expected_events = ExpectedSequence::new([
        vec![lsp_change_event!("\n")],
        vec![lsp_change_event!(0:0..0:0 => "©")],
        // This would be 0:2 if utf-8
        vec![lsp_change_event!(0:1..0:1 => "z")],
    ]);

    cx.setup_lang_server(zi::filetype!(text), "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(move |_, _params| async {
                Ok(lsp_types::InitializeResult {
                    capabilities: lsp_types::ServerCapabilities {
                        position_encoding: Some(lsp_types::PositionEncodingKind::UTF16),
                        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
                            lsp_types::TextDocumentSyncKind::INCREMENTAL,
                        )),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            })
            .notification::<notification::DidChangeTextDocument>(move |_, params| {
                expected_events.assert_eq(&params.content_changes);
                Ok(())
            })
    })
    .await;

    let buf = cx.open_tmp("", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;

    cx.with(move |editor| {
        editor.edit(buf, &deltas![0..0 => "©"])?;
        editor.edit(buf, &deltas![2..2 => "z"])
    })
    .await?;

    cx.cleanup().await;

    Ok(())
}

#[tokio::test]
async fn lsp_changes_incremental_utf8_random() -> zi::Result<()> {
    let cx = new("").await;

    let (tx, rx) = watch::channel(String::new());

    cx.setup_lang_server(zi::filetype!(text), "test-server", tx, |builder| {
        builder
            .request::<request::Initialize, _>(move |_, _params| async {
                Ok(lsp_types::InitializeResult {
                    capabilities: lsp_types::ServerCapabilities {
                        position_encoding: Some(lsp_types::PositionEncodingKind::UTF8),
                        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
                            lsp_types::TextDocumentSyncKind::INCREMENTAL,
                        )),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            })
            .notification::<notification::DidChangeTextDocument>(move |tx, params| {
                for change in params.content_changes {
                    match change.range {
                        None => tx.send_modify(|text| text.replace_range(.., &change.text)),
                        Some(lsp_range) => {
                            let text = tx.borrow();
                            let point_range = zi_lsp::from_proto::range(
                                lstypes::PositionEncoding::Utf8,
                                &*text,
                                lsp_range,
                            )
                            .unwrap();
                            let byte_range = text.point_range_to_byte_range(point_range);
                            // Must drop the guard, otherwise `send` will be stuck on a lock
                            drop(text);
                            tx.send_modify(|text| text.replace_range(byte_range, &change.text));
                        }
                    }
                }
                Ok(())
            })
    })
    .await;

    let buf = cx.open_tmp("", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;

    let mut any_non_empty = false;
    for _ in 0..rand::rng().random_range(0..400) {
        any_non_empty |= !cx
            .with(move |editor| editor.apply_random_deltas(rand::rng(), buf))
            .await
            .is_empty();
    }

    if !any_non_empty {
        panic!("no non-empty deltas were applied");
    }

    assert_eq!(
        *rx.borrow(),
        cx.with(move |editor| editor.text(buf).to_string()).await,
        "editor state and lsp state have diverged"
    );

    cx.cleanup().await;

    Ok(())
}
