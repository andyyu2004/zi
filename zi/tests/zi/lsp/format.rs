use zi_lsp::lsp_types::{self, notification, request, OneOf};

use super::*;
use crate::lsp::fake::FakeLanguageServer;

#[tokio::test]
async fn lsp_format() -> zi::Result<()> {
    let cx = new_cx(zi::Size::new(10, 5), "").await;

    let path = cx.tempfile(r#"abc"#)?;

    let buf = cx
        .with(|editor| {
            let server_id = zi::LanguageServerId::new("test");

            let server = FakeLanguageServer::builder()
                .request::<request::Initialize, _>(|_st: &mut (), _req| async move {
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
                .notification::<notification::Initialized>(|_st: &mut (), _params| Ok(()))
                .notification::<notification::DidOpenTextDocument>(|_st: &mut (), _params| Ok(()))
                .notification::<notification::DidChangeTextDocument>(|_st: &mut (), _params| Ok(()))
                .finish(());

            editor
                .language_config_mut()
                .add_language(zi::FileType::TEXT, zi::LanguageConfig::new([server_id]))
                .add_language_server(server_id, server);

            let buf = editor.open(path, zi::OpenFlags::SPAWN_LANGUAGE_SERVERS)?;
            Ok::<_, zi::Error>(buf)
        })
        .await?;

    let save_fut = cx.with(move |editor| editor.save(buf)).await;

    cx.with(move |editor| assert_eq!(editor[buf].text().to_string(), "abc")).await;
    let () = save_fut.await?;
    cx.with(move |editor| assert_eq!(editor[buf].text().to_string(), "def\n")).await;

    Ok(())
}
