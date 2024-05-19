use zi::Url;

use super::*;

#[tokio::test]
async fn lsp_definition() -> zi::Result<()> {
    let cx = new_cx("").await;

    let path = cx.tempfile(
        r#"abc
def
ghi"#,
    )?;
    let uri = Url::from_file_path(&path).unwrap();

    cx.setup_lang_server(zi::FileType::TEXT, "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(|_, _| async {
                Ok(lsp_types::InitializeResult {
                    capabilities: lsp_types::ServerCapabilities {
                        position_encoding: Some(lsp_types::PositionEncodingKind::UTF8),
                        definition_provider: Some(OneOf::Left(true)),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            })
            .request::<request::GotoDefinition, _>(move |_st: &mut (), _params| {
                let uri = uri.clone();
                async move {
                    Ok(Some(lsp_types::GotoDefinitionResponse::Scalar(lsp_types::Location {
                        uri,
                        range: lsp_range!(2:1..2:3),
                    })))
                }
            })
    })
    .await;

    cx.with(move |editor| {
        editor.open(path, zi::OpenFlags::ACTIVE | zi::OpenFlags::SPAWN_LANGUAGE_SERVERS)
    })
    .await?;
    cx.with(move |editor| editor.goto_definition(zi::Active)).await;
    cx.with(|editor| assert_eq!(editor.view(zi::Active).cursor(), zi::Point::new(2, 1))).await;

    cx.cleanup().await;
    Ok(())
}
