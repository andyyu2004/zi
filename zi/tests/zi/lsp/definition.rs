use zi::{BufferId, Url};

use super::*;

async fn setup(
    cx: &TestContext,
    position_encoding: lsp_types::PositionEncodingKind,
    range: lsp_types::Range,
    text: &'static str,
) -> zi::Result<BufferId> {
    let path = cx.tempfile(text)?;
    let uri = Url::from_file_path(&path).unwrap();

    cx.setup_lang_server(zi::FileType::TEXT, "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(move |_, _| {
                let value = position_encoding.clone();
                async move {
                    Ok(lsp_types::InitializeResult {
                        capabilities: lsp_types::ServerCapabilities {
                            position_encoding: Some(value),
                            definition_provider: Some(OneOf::Left(true)),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                }
            })
            .request::<request::GotoDefinition, _>(move |_st: &mut (), _params| {
                let uri = uri.clone();
                async move {
                    Ok(Some(lsp_types::GotoDefinitionResponse::Scalar(lsp_types::Location {
                        uri,
                        range,
                    })))
                }
            })
    })
    .await;

    let buf = cx.open(&path, zi::OpenFlags::SPAWN_LANGUAGE_SERVERS).await?;
    cx.with(move |editor| {
        assert_eq!(editor.buffer(zi::Active).id(), buf);
        assert_eq!(editor[buf].text().to_string(), text);
    })
    .await;
    Ok(buf)
}

#[tokio::test]
async fn lsp_definition_utf8() -> zi::Result<()> {
    let cx = new("").await;

    setup(&cx, lsp_types::PositionEncodingKind::UTF8, lsp_range!(0:4..0:5), r#"ab©de"#).await?;

    cx.with(move |editor| editor.goto_definition(zi::Active)).await;
    cx.with(|editor| assert_eq!(editor.view(zi::Active).cursor(), zi::Point::new(0, 4))).await;

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn lsp_definition_utf16() -> zi::Result<()> {
    let cx = new("").await;

    setup(&cx, lsp_types::PositionEncodingKind::UTF16, lsp_range!(0:3..0:4), r#"ab©de"#).await?;

    cx.with(move |editor| editor.goto_definition(zi::Active)).await;
    cx.with(|editor| assert_eq!(editor.view(zi::Active).cursor(), zi::Point::new(0, 4))).await;

    cx.cleanup().await;
    Ok(())
}
