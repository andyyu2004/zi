use stdx::bomb::DropBomb;

use super::*;

#[tokio::test]
async fn lsp_change_no_sync() -> zi::Result<()> {
    let cx = new_cx("").await;
    let bomb = DropBomb::new("initialize should be called");

    cx.setup_lang_server(zi::FileType::TEXT, "test-server", (), |builder| {
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

    let buf = cx.open("", zi::OpenFlags::ACTIVE | zi::OpenFlags::SPAWN_LANGUAGE_SERVERS).await?;
    cx.with(move |editor| editor.edit(buf, &zi::Deltas::insert_at(0, "abc"))).await;

    Ok(())
}

#[tokio::test]
async fn lsp_change_full_sync() -> zi::Result<()> {
    let cx = new_cx("").await;
    let bomb = DropBomb::new("did_change should be called");

    let expected_events = ExpectedSequence::new([
        vec![lsp_change_event!("dbc\n")],
        // should probably try and merge some of these events into 1
        vec![lsp_change_event!("abc\n")],
        vec![lsp_change_event!("abc")],
        vec![lsp_change_event!("abc\n")],
        vec![lsp_change_event!("dbc\n")],
    ]);

    cx.setup_lang_server(zi::FileType::TEXT, "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(|_, _| async {
                Ok(lsp_types::InitializeResult {
                    capabilities: lsp_types::ServerCapabilities {
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
    let deltas = zi::lsp::from_proto::deltas(text, edits);
    let buf = cx.open(text, zi::OpenFlags::ACTIVE | zi::OpenFlags::SPAWN_LANGUAGE_SERVERS).await?;
    cx.with(move |editor| editor.edit(buf, &deltas)).await;

    assert!(cx.with(move |editor| editor.undo(buf)).await);

    assert!(cx.with(move |editor| editor.redo(buf)).await);

    Ok(())
}

#[tokio::test]
async fn lsp_changes_incremental_utf8() -> zi::Result<()> {
    let cx = new_cx("").await;

    lsp_range!(0:0..0:0);

    let expected_events = ExpectedSequence::new([
        vec![lsp_change_event!(0:0..0:0 => "abc")],
        vec![
            lsp_change_event!(0:3..0:3 => "d"),
            // FIXME should be 5
            lsp_change_event!(0:4..0:4 => "e"),
        ],
    ]);

    cx.setup_lang_server(zi::FileType::TEXT, "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(move |_, params| async {
                assert!(
                    params
                        .capabilities
                        .general
                        .unwrap()
                        .position_encodings
                        .unwrap()
                        .contains(&lsp_types::PositionEncodingKind::UTF8),
                );

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
            .notification::<notification::DidChangeTextDocument>(move |_st, params| {
                expected_events.assert_eq(&params.content_changes);
                Ok(())
            })
    })
    .await;

    let buf = cx.open("", zi::OpenFlags::ACTIVE | zi::OpenFlags::SPAWN_LANGUAGE_SERVERS).await?;
    cx.with(move |editor| editor.edit(buf, &zi::Deltas::insert_at(0, "abc"))).await;

    cx.with(move |editor| {
        editor.edit(buf, &zi::Deltas::new([zi::Delta::new(3..3, "d"), zi::Delta::new(4..4, "e")]))
    })
    .await;

    Ok(())
}
