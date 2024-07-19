use std::path::PathBuf;

use zi::{deltas, lstypes, BufferId};
use zi_lsp::lsp_types::{self, request};
use zi_test::{new, TestContext};

use crate::TestContextExt;

async fn setup(
    cx: &TestContext,
    position_encoding: lsp_types::PositionEncodingKind,
    text: &'static str,
    diagnostics: lsp_types::DocumentDiagnosticReportResult,
) -> zi::Result<(BufferId, PathBuf)> {
    let path = cx.tempfile(text)?;

    cx.setup_lang_server(zi::filetype!(text), "test-server", (), |builder| {
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

    let buf = cx.open(&path, zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
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

    cx.with(move |editor| {
        // edit the buffer to bump the version
        assert_eq!(editor.buffer(buf).version(), 0);
        editor.edit(buf, &deltas![ 0..0 => "text" ]).unwrap();
        assert_eq!(editor.buffer(buf).version(), 1);
        editor.request_diagnostics(buf)
    })
    .await
    .await?;

    let expected_main_diagnostics = main_diagnostics
        .into_iter()
        .filter_map(|diag| zi_lsp::from_proto::diagnostic(lstypes::PositionEncoding::Utf8, diag))
        .collect();

    let expected_related_diagnostics = related_diagnostics
        .into_iter()
        .filter_map(|diag| zi_lsp::from_proto::diagnostic(lstypes::PositionEncoding::Utf8, diag))
        .collect();

    assert_eq!(
        cx.with(move |editor| editor.diagnostics().clone()).await,
        zi::hashmap! {
            path => zi::Setting::new((1, expected_main_diagnostics)),
            PathBuf::from("/related") => zi::Setting::new((0, expected_related_diagnostics)),
        }
    );

    cx.cleanup().await;

    Ok(())
}
