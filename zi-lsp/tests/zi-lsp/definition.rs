use tokio::sync::mpsc;
use zi::event::HandlerResult;
use zi::{BufferId, LanguageConfig, Url};
use zi_lsp::LanguageServerConfig;

use super::*;

async fn setup(
    cx: &TestContext,
    position_encoding: lsp_types::PositionEncodingKind,
    range: lsp_types::Range,
    text: &'static str,
) -> zi::Result<BufferId> {
    let path = cx.tempfile(text)?;
    let uri = Url::from_file_path(&path).unwrap();

    cx.setup_lang_server(zi::filetype!(text), "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(move |_, _| {
                let encoding = position_encoding.clone();
                async move {
                    Ok(lsp_types::InitializeResult {
                        capabilities: lsp_types::ServerCapabilities {
                            position_encoding: Some(encoding),
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

    let buf = cx.open(&path, zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
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

    cx.with(move |editor| editor.goto_definition(zi::Active)).await.await?;
    cx.with(|editor| assert_eq!(editor.view(zi::Active).cursor(), zi::Point::new(0, 4))).await;

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn lsp_definition_utf16() -> zi::Result<()> {
    let cx = new("").await;

    setup(&cx, lsp_types::PositionEncodingKind::UTF16, lsp_range!(0:3..0:4), r#"ab©de"#).await?;

    cx.with(move |editor| editor.goto_definition(zi::Active)).await.await?;
    cx.with(|editor| assert_eq!(editor.view(zi::Active).cursor(), zi::Point::new(0, 4))).await;

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn lsp_definition_cross_file() -> zi::Result<()> {
    let cx = new("").await;

    cx.with(|editor| {
        editor
            .language_config_mut()
            .add_language(zi::filetype!(go), LanguageConfig::new(["gopls".into()]))
            .add_language_service("gopls", LanguageServerConfig::new("gopls", []));
    })
    .await;

    let (tx, mut rx) = mpsc::unbounded_channel();
    zi::event::subscribe_with::<zi::event::DidInitializeLanguageService>(move |_editor, _event| {
        tx.send(()).unwrap();
        HandlerResult::Continue
    });

    cx.with(move |editor| {
        editor
            .open("tests/zi-lsp/testdirs/lsp-test/main.go", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES)
    })
    .await?
    .await?;

    rx.recv().await;

    cx.with(move |editor| {
        assert_eq!(editor.cursor_line(), "// An example go module to test lsp interactions");
        editor.set_cursor(zi::Active, zi::Point::new(4, 1));
        assert_eq!(editor.cursor_char(), Some('f'));
        editor.goto_definition(zi::Active)
    })
    .await
    .await?;

    cx.with(|editor| {
        assert_eq!(editor.cursor_line(), "func f(i int) {");
        // FIXME this is necessary since we don't deal with tabs properly
        editor.input("jB").unwrap();
        assert_eq!(editor.cursor_line(), "\tg(i)");
        assert_eq!(editor.cursor_char(), Some('g'));
        editor.goto_definition(zi::Active)
    })
    .await
    .await?;

    cx.with(|editor| {
        assert_eq!(editor.cursor_line(), "func g(int) {");
    })
    .await;

    cx.cleanup().await;
    Ok(())
}
