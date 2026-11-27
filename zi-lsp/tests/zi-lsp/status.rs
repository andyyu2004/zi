use std::time::Duration;

use zi::lstypes::{PositionEncoding, ServerInfo};
use zi::{
    LanguageConfig, LanguageServiceId, LanguageServiceState, LanguageServiceStatus,
    ServiceCapabilities, ServiceDetails,
};

use super::*;

async fn status(cx: &TestContext, id: LanguageServiceId) -> LanguageServiceStatus {
    cx.with(move |editor| editor.language_services())
        .await
        .into_iter()
        .find(|status| status.id == id)
        .unwrap_or_else(|| panic!("no status for `{id}`"))
}

async fn wait_for_state(
    cx: &TestContext,
    id: LanguageServiceId,
    done: impl Fn(&LanguageServiceState) -> bool,
) -> LanguageServiceState {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let LanguageServiceStatus { state, .. } = status(cx, id).await;
        if done(&state) {
            return state;
        }
        assert!(std::time::Instant::now() < deadline, "service `{id}` is still {state:?}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn lsp_status_running_service() -> zi::Result<()> {
    let cx = new("").await;
    let service_id = LanguageServiceId::from("status-running");

    cx.setup_lang_server(zi::filetype!(text), service_id, (), |builder| {
        builder.request::<request::Initialize, _>(|(), _| async {
            Ok(lsp_types::InitializeResult {
                capabilities: lsp_types::ServerCapabilities {
                    position_encoding: Some(lsp_types::PositionEncodingKind::UTF8),
                    definition_provider: Some(OneOf::Left(true)),
                    references_provider: Some(OneOf::Left(true)),
                    ..Default::default()
                },
                server_info: Some(lsp_types::ServerInfo {
                    name: "fake".into(),
                    version: Some("1.2.3".into()),
                }),
            })
        })
    })
    .await;
    cx.with(|editor| {
        editor
            .language_config_mut()
            .add_language(zi::filetype!(json), LanguageConfig::new(["undefined".into()]));
    })
    .await;

    let services = cx.with(|editor| editor.language_services()).await;
    assert_eq!(
        services,
        [
            LanguageServiceStatus {
                id: service_id,
                state: LanguageServiceState::NotStarted,
                command: None,
                filetypes: [zi::filetype!(text)].into(),
            },
            LanguageServiceStatus {
                id: "undefined".into(),
                state: LanguageServiceState::Undefined,
                command: None,
                filetypes: [zi::filetype!(json)].into(),
            },
        ]
    );

    let mut initialized = on_initialized(service_id);
    cx.open_tmp("abc", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
    initialized.recv().await.unwrap();

    assert_eq!(
        status(&cx, service_id).await.state,
        LanguageServiceState::Running(ServiceDetails {
            server_info: Some(ServerInfo { name: "fake".into(), version: Some("1.2.3".into()) }),
            process_id: None,
            root: std::env::current_dir()?,
            position_encoding: Some(PositionEncoding::Utf8),
            capabilities: ServiceCapabilities::DEFINITION | ServiceCapabilities::REFERENCES,
        })
    );

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn lsp_status_failed_initialize() -> zi::Result<()> {
    let cx = new("").await;
    let service_id = LanguageServiceId::from("status-failing");

    cx.setup_lang_server(zi::filetype!(text), service_id, (), |builder| {
        builder.request::<request::Initialize, _>(|(), _| async {
            Err(ResponseError::new(ErrorCode::INTERNAL_ERROR, "no workspace").into())
        })
    })
    .await;

    cx.open_tmp("", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
    let state =
        wait_for_state(&cx, service_id, |state| matches!(state, LanguageServiceState::Failed(_)))
            .await;
    let LanguageServiceState::Failed(err) = state else { unreachable!() };
    assert!(err.contains("no workspace"), "{err}");

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn lsp_status_detects_exited_server() -> zi::Result<()> {
    let cx = new("").await;
    let service_id = LanguageServiceId::from("gopls");
    start_gopls(&cx, &[]).await?;

    let LanguageServiceState::Running(details) = status(&cx, service_id).await.state else {
        panic!("gopls should be running");
    };
    let pid = details.process_id.expect("gopls runs as a process");
    assert_eq!(details.server_info.map(|info| info.name).as_deref(), Some("gopls"));

    let killed = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status()?;
    assert!(killed.success());

    let state =
        wait_for_state(&cx, service_id, |state| matches!(state, LanguageServiceState::Exited(_)))
            .await;
    let LanguageServiceState::Exited(details) = state else { unreachable!() };
    assert_eq!(details.process_id, Some(pid));

    cx.cleanup().await;
    Ok(())
}
