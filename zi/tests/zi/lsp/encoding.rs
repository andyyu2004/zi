use super::*;

#[tokio::test]
async fn default_to_utf16() -> zi::Result<()> {
    let cx = new_cx("").await;

    cx.setup_lang_server(zi::FileType::TEXT, "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(move |_, params| async {
                assert_eq!(
                    params.capabilities.general.and_then(|g| g.position_encodings),
                    Some(vec![
                        lsp_types::PositionEncodingKind::UTF8,
                        lsp_types::PositionEncodingKind::UTF16,
                        lsp_types::PositionEncodingKind::UTF32,
                    ])
                );

                Ok(lsp_types::InitializeResult {
                    capabilities: lsp_types::ServerCapabilities {
                        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
                            lsp_types::TextDocumentSyncKind::INCREMENTAL,
                        )),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            })
            .notification::<notification::DidChangeTextDocument>(move |_st, params| Ok(()))
    })
    .await;

    let buf = cx.open("", zi::OpenFlags::ACTIVE | zi::OpenFlags::SPAWN_LANGUAGE_SERVERS).await?;
    cx.with(move |editor| editor.edit(buf, &zi::Deltas::insert_at(0, "abc"))).await;

    Ok(())
}
