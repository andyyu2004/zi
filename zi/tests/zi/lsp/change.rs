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
                assert_eq!(
                    params.content_changes,
                    vec![lsp_types::TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: "dbc\n".to_string(),
                    }]
                );
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

    Ok(())
}
