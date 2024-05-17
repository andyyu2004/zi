use tokio::fs;
use zi_lsp::lsp_types::{self, request, OneOf};

use super::*;

#[tokio::test]
async fn lsp_format() -> zi::Result<()> {
    let cx = new_cx(zi::Size::new(10, 5), "").await;

    let path = cx.tempfile(r#"abc"#)?;

    cx.setup_lang_server(zi::FileType::TEXT, "test-server", (), |builder| {
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
            .request::<request::Formatting, _>(|_st: &mut (), _req| async move {
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

    let buf = cx
        .with({
            let path = path.to_path_buf();
            move |editor| editor.open_active(path)
        })
        .await?;

    let save_fut = cx.with(move |editor| editor.save(buf, zi::SaveFlags::empty())).await;

    assert_eq!(fs::read_to_string(&path).await?, "abc");
    cx.with(move |editor| assert_eq!(editor[buf].text().to_string(), "abc")).await;

    let () = save_fut.await?;
    cx.with(move |editor| assert_eq!(editor[buf].text().to_string(), "def\n")).await;
    assert_eq!(fs::read_to_string(&path).await?, "def\n");

    cx.with(move |editor| {
        assert!(editor.undo(zi::Active));
        assert_eq!(editor[buf].text().to_string(), "abc");
        assert!(editor[buf].flags().contains(zi::BufferFlags::DIRTY));
    })
    .await;

    Ok(())
}
