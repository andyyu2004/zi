use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use zi::event::HandlerResult;
use zi::{LanguageConfig, LanguageServiceId, Url};
use zi_lsp::LanguageServerConfig;

use super::*;

fn on_initialized(service_id: LanguageServiceId) -> mpsc::UnboundedReceiver<()> {
    let (tx, rx) = mpsc::unbounded_channel();
    zi::event::subscribe_with::<zi::event::DidInitializeLanguageService>(move |_editor, event| {
        if event.service_id == service_id {
            let _ = tx.send(());
        }
        HandlerResult::Continue
    });
    rx
}

async fn wait_for_error(cx: &TestContext) -> String {
    loop {
        if let Some(err) = cx.with(|editor| editor.get_error().map(str::to_owned)).await {
            return err;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn lsp_concurrent_open_spawns_once() -> zi::Result<()> {
    let cx = new("").await;
    let service_id = LanguageServiceId::from("startup-concurrent");

    let (init_tx, init_rx) = watch::channel(false);
    let inits = Arc::new(AtomicUsize::new(0));
    let opened = Arc::new(Mutex::new(Vec::<Url>::new()));

    cx.setup_lang_server(zi::filetype!(text), service_id, (), {
        let inits = Arc::clone(&inits);
        let opened = Arc::clone(&opened);
        move |builder| {
            builder
                .request::<request::Initialize, _>(move |(), _| {
                    inits.fetch_add(1, atomic::Ordering::SeqCst);
                    let mut rx = init_rx.clone();
                    async move {
                        let _ = rx.wait_for(|&ready| ready).await;
                        Ok(lsp_types::InitializeResult::default())
                    }
                })
                .notification::<notification::DidOpenTextDocument>(move |_st, params| {
                    opened.lock().unwrap().push(params.text_document.uri);
                    Ok(())
                })
        }
    })
    .await;

    let mut initialized = on_initialized(service_id);
    let a = cx.tempfile("a")?;
    let b = cx.tempfile("b")?;
    cx.open(&a, zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
    cx.open(&b, zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;

    init_tx.send(true).unwrap();
    initialized.recv().await.unwrap();

    assert_eq!(inits.load(atomic::Ordering::SeqCst), 1, "expected exactly one server spawn");
    let opened = opened.lock().unwrap().clone();
    for path in [&a, &b] {
        let url = Url::from_file_path(path).unwrap();
        assert!(opened.contains(&url), "missing didOpen for {url}: {opened:?}");
    }

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn lsp_shared_service_across_filetypes() -> zi::Result<()> {
    let cx = new("").await;
    let service_id = LanguageServiceId::from("startup-shared");

    let inits = Arc::new(AtomicUsize::new(0));
    let definitions = Arc::new(Mutex::new(Vec::<Url>::new()));

    let server = FakeLanguageServer::builder()
        .request::<request::Initialize, _>({
            let inits = Arc::clone(&inits);
            move |(), _| {
                inits.fetch_add(1, atomic::Ordering::SeqCst);
                async {
                    Ok(lsp_types::InitializeResult {
                        capabilities: lsp_types::ServerCapabilities {
                            definition_provider: Some(OneOf::Left(true)),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                }
            }
        })
        .notification::<notification::Initialized>(|_st, _params| Ok(()))
        .notification::<notification::DidOpenTextDocument>(|_st, _params| Ok(()))
        .request::<request::GotoDefinition, _>({
            let definitions = Arc::clone(&definitions);
            move |_st: &mut (), params| {
                let uri = params.text_document_position_params.text_document.uri;
                definitions.lock().unwrap().push(uri.clone());
                async move {
                    Ok(Some(lsp_types::GotoDefinitionResponse::Scalar(lsp_types::Location {
                        uri,
                        range: lsp_range!(0:0..0:0),
                    })))
                }
            }
        })
        .finish(());

    cx.with(move |editor| {
        editor
            .language_config_mut()
            .add_language(zi::filetype!(javascript), LanguageConfig::new([service_id]))
            .add_language(zi::filetype!(typescript), LanguageConfig::new([service_id]))
            .add_language_service(service_id, server);
    })
    .await;

    let dir = cx.tempdir()?;
    let js = dir.join("a.js");
    let ts = dir.join("b.ts");
    std::fs::write(&js, "a")?;
    std::fs::write(&ts, "b")?;

    let mut initialized = on_initialized(service_id);
    cx.open(&js, zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
    initialized.recv().await.unwrap();

    cx.open(&ts, zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
    cx.with(move |editor| editor.goto_definition(zi::Active))
        .await
        .await
        .expect("goto definition in the typescript buffer should reach the shared server");

    assert_eq!(inits.load(atomic::Ordering::SeqCst), 1);
    assert_eq!(*definitions.lock().unwrap(), [Url::from_file_path(&ts).unwrap()]);

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn lsp_undefined_service_is_error() -> zi::Result<()> {
    let cx = new("").await;

    cx.with(|editor| {
        editor
            .language_config_mut()
            .add_language(zi::filetype!(text), LanguageConfig::new(["startup-undefined".into()]));
    })
    .await;

    let err = cx
        .open_tmp("", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES)
        .await
        .expect_err("opening with an undefined language service should fail");
    assert!(err.to_string().contains("startup-undefined"), "{err}");

    cx.cleanup().await;
    Ok(())
}

#[tokio::test]
async fn lsp_failed_initialize_can_retry() -> zi::Result<()> {
    let cx = new("").await;
    let service_id = LanguageServiceId::from("startup-retry");
    let inits = Arc::new(AtomicUsize::new(0));

    cx.setup_lang_server(zi::filetype!(text), service_id, (), {
        let inits = Arc::clone(&inits);
        move |builder| {
            builder.request::<request::Initialize, _>(move |(), _| {
                let n = inits.fetch_add(1, atomic::Ordering::SeqCst);
                async move {
                    if n == 0 {
                        Err(ResponseError::new(ErrorCode::INTERNAL_ERROR, "first init fails")
                            .into())
                    } else {
                        Ok(lsp_types::InitializeResult::default())
                    }
                }
            })
        }
    })
    .await;

    let mut initialized = on_initialized(service_id);
    cx.open_tmp("", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
    let err = wait_for_error(&cx).await;
    assert!(err.contains("first init fails"), "{err}");

    cx.open_tmp("", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;
    initialized.recv().await.unwrap();
    assert_eq!(inits.load(atomic::Ordering::SeqCst), 2);

    cx.cleanup().await;
    Ok(())
}

fn process_exists(pid: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", r#"kill -0 "$0""#, pid])
        .stderr(std::process::Stdio::null())
        .status()
        .expect("failed to run kill")
        .success()
}

// A real process that never answers `initialize`. Time is paused, so the init timeout fires as
// soon as the editor is idle, and the test checks the process is killed rather than leaked.
#[tokio::test(start_paused = true)]
async fn lsp_initialize_timeout_kills_server() -> zi::Result<()> {
    let cx = new("").await;
    let service_id = LanguageServiceId::from("startup-hang");
    let pidfile: PathBuf = cx.tempdir()?.join("pid");

    cx.with({
        let pidfile = pidfile.clone();
        move |editor| {
            editor
                .language_config_mut()
                .add_language(zi::filetype!(text), LanguageConfig::new([service_id]))
                .add_language_service(
                    service_id,
                    LanguageServerConfig::new(
                        "sh",
                        [
                            "-c".into(),
                            r#"echo $$ > "$0.tmp" && mv "$0.tmp" "$0" && exec sleep 600"#.into(),
                            pidfile.into(),
                        ],
                    ),
                );
        }
    })
    .await;

    cx.open_tmp("", zi::OpenFlags::SPAWN_LANGUAGE_SERVICES).await?;

    // Block the runtime (so paused time can't advance) until the server has written its pid.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let pid = loop {
        if let Ok(pid) = std::fs::read_to_string(&pidfile) {
            break pid.trim().to_owned();
        }
        assert!(std::time::Instant::now() < deadline, "server never wrote its pid");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(process_exists(&pid));

    let err = wait_for_error(&cx).await;
    assert!(err.contains("timed out"), "{err}");

    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while process_exists(&pid) {
        assert!(std::time::Instant::now() < deadline, "timed out server {pid} is still running");
        std::thread::sleep(Duration::from_millis(10));
    }

    cx.cleanup().await;
    Ok(())
}
