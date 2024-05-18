use super::*;

#[tokio::test]
async fn lsp_change() -> zi::Result<()> {
    cov_mark::check_count!(test_lsp_change, 1);

    let cx = new_cx("").await;

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
            .notification::<notification::DidChangeTextDocument>(|_st, params| {
                cov_mark::hit!(test_lsp_change);
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
