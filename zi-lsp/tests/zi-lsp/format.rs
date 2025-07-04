use tokio::fs;

use super::*;

#[tokio::test]
async fn lsp_format() -> zi::Result<()> {
    let cx = new("").await;

    let path = cx.tempfile(r#"abc"#)?;

    cx.setup_lang_server(zi::filetype!(text), "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(|_, _| async {
                Ok(lsp_types::InitializeResult {
                    capabilities: lsp_types::ServerCapabilities {
                        document_formatting_provider: Some(OneOf::Left(true)),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            })
            .request::<request::Formatting, _>(|_st: &mut (), _params| async move {
                Ok(Some(vec![lsp_types::TextEdit {
                    range: lsp_types::Range {
                        start: lsp_types::Position { line: 0, character: 0 },
                        end: lsp_types::Position { line: 0, character: 3 },
                    },
                    new_text: "def".to_string(),
                }]))
            })
    })
    .await;

    let buf = cx.open(&path, zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;

    let save_fut = cx.with(move |editor| editor.save(buf, zi::SaveFlags::empty())).await;

    assert_eq!(fs::read_to_string(&path).await?, "abc");
    cx.with(move |editor| assert_eq!(editor[buf].text().to_string(), "abc")).await;

    let () = save_fut.await?;
    cx.with(move |editor| assert_eq!(editor[buf].text().to_string(), "def\n")).await;
    assert_eq!(fs::read_to_string(&path).await?, "def\n");

    cx.with(move |editor| {
        assert!(editor.undo(zi::Active).unwrap());
        assert_eq!(editor[buf].text().to_string(), "abc");
        assert!(editor[buf].flags().contains(zi::BufferFlags::DIRTY));
    })
    .await;

    cx.cleanup().await;
    Ok(())
}
