use std::path::PathBuf;

use zi::BufferId;
use zi_text::deltas;

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
    let main_diagnostics = vec![lsp_types::Diagnostic {
        range: lsp_types::Range {
            start: lsp_types::Position { line: 0, character: 0 },
            end: lsp_types::Position { line: 0, character: 0 },
        },
        ..Default::default()
    }];

    let related_diagnostics = vec![lsp_types::Diagnostic {
        range: lsp_types::Range {
            start: lsp_types::Position { line: 1, character: 0 },
            end: lsp_types::Position { line: 1, character: 0 },
        },
        ..Default::default()
    }];

    let res = lsp_types::DocumentDiagnosticReportResult::Report(
        lsp_types::DocumentDiagnosticReport::Full(lsp_types::RelatedFullDocumentDiagnosticReport {
            full_document_diagnostic_report: lsp_types::FullDocumentDiagnosticReport {
                items: main_diagnostics.clone(),
                ..Default::default()
            },
            related_documents: Some(zi::hashmap! {
                zi::Url::parse("file:///related").unwrap() => lsp_types::DocumentDiagnosticReportKind::Full(
                    lsp_types::FullDocumentDiagnosticReport {
                        items: related_diagnostics.clone(),
                        ..Default::default()
                    }
                )
            }),
        }),
    );

    let (buf, path) = setup(&cx, lsp_types::PositionEncodingKind::UTF8, "\n", res).await?;

    let server = zi::LanguageServerId::new("test-server");

    cx.with(move |editor| {
        // edit the buffer to bump the version
        assert_eq!(editor.buffer(buf).version(), 0);
        editor.edit(buf, &deltas![ 0..0 => "text" ]).unwrap();
        assert_eq!(editor.buffer(buf).version(), 1);
        editor.request_diagnostics(buf)
    })
    .await
    .await?;
    assert_eq!(
        cx.with(move |editor| editor.lsp_diagnostics().clone()).await,
        zi::hashmap! {
            path => zi::hashmap! {
                server => zi::Setting::new((1, main_diagnostics.into())),
            },
            PathBuf::from("/related") => zi::hashmap! {
                server => zi::Setting::new((0, related_diagnostics.into())),
            }
        }
    );

    cx.cleanup().await;

    Ok(())
}
