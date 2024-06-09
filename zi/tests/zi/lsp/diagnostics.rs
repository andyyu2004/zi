use zi::BufferId;

use super::*;
use crate::new;

async fn setup(
    cx: &TestContext,
    position_encoding: lsp_types::PositionEncodingKind,
    text: &'static str,
    diagnostics: lsp_types::DocumentDiagnosticReportResult,
) -> zi::Result<(BufferId, PathBuf)> {
    let path = cx.tempfile(text)?;

    cx.setup_lang_server(zi::FileType::TEXT, "test-server", (), |builder| {
        builder
            .request::<request::Initialize, _>(move |_, _| {
                let encoding = position_encoding.clone();
                async move {
                    Ok(lsp_types::InitializeResult {
                        capabilities: lsp_types::ServerCapabilities {
                            position_encoding: Some(encoding),
                            diagnostic_provider: Some(
                                lsp_types::DiagnosticServerCapabilities::Options(
                                    lsp_types::DiagnosticOptions {
                                        identifier: None,
                                        inter_file_dependencies: true,
                                        workspace_diagnostics: true,
                                        work_done_progress_options: Default::default(),
                                    },
                                ),
                            ),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                }
            })
            .request::<request::DocumentDiagnosticRequest, _>(move |_st: &mut (), _params| {
                let diagnostics = diagnostics.clone();
                async { Ok(diagnostics) }
            })
    })
    .await;

    let buf = cx.open(&path, zi::OpenFlags::SPAWN_LANGUAGE_SERVERS).await?;
    cx.with(move |editor| editor.request_diagnostics(buf)).await.await?;
    Ok((buf, path))
}

#[tokio::test]
async fn lsp_pull_diagnostics() -> zi::Result<()> {
    let cx = new("").await;
    let diagnostics = vec![
        lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position { line: 0, character: 0 },
                end: lsp_types::Position { line: 0, character: 0 },
            },
            ..Default::default()
        },
        lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position { line: 1, character: 0 },
                end: lsp_types::Position { line: 1, character: 2 },
            },
            ..Default::default()
        },
    ];

    let res = lsp_types::DocumentDiagnosticReportResult::Report(
        lsp_types::DocumentDiagnosticReport::Full(lsp_types::RelatedFullDocumentDiagnosticReport {
            full_document_diagnostic_report: lsp_types::FullDocumentDiagnosticReport {
                items: diagnostics.clone(),
                ..Default::default()
            },
            related_documents: Default::default(),
        }),
    );

    let (buf, path) = setup(&cx, lsp_types::PositionEncodingKind::UTF8, "", res).await?;

    cx.with(move |editor| editor.request_diagnostics(buf)).await.await?;
    assert_eq!(
        cx.with(move |editor| editor.lsp_diagnostics().clone()).await,
        zi::hashmap! {
            path => zi::hashmap! {
                zi::LanguageServerId::new("test-server") => zi::Setting::new((0, diagnostics.into())),
            }
        }
    );

    cx.cleanup().await;

    Ok(())
}
