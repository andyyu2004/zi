use std::backtrace::Backtrace;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use crossterm::event::EventStream;
use futures_util::StreamExt;
use tokio::sync::Notify;
use tracing_subscriber::EnvFilter;
use tui::Terminal;
use tui::backend::CrosstermBackend;
use zi::input::Event;

#[derive(Parser)]
struct Opts {
    #[clap(long)]
    log: Option<PathBuf>,
    path: Option<PathBuf>,
    #[clap(long)]
    readonly: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    let mut opts = Opts::parse();

    const ZI_LOG: &str = "ZI_LOG";

    if std::env::var(ZI_LOG).is_ok() && opts.log.is_none() {
        let state = zi::dirs::state();
        std::fs::create_dir_all(&state)?;
        opts.log = Some(state.join("zi.log"));
    }

    if let Some(log) = opts.log {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(log)?;
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .with_env_filter(EnvFilter::from_env(ZI_LOG))
            .init();
    }

    let stdout = io::stdout().lock();
    let term = Terminal::new(CrosstermBackend::new(stdout))?;
    let size = term.size()?;
    let (mut editor, tasks) =
        zi::Editor::new(zi_wasm::WasmBackend::default(), (size.width, size.height));

    let plugins = zi_wasm::PluginManager::default();
    let plugins_initialized = plugins.initialized();
    assert!(editor.register_plugin_manager(plugins).is_none());

    let init_path = zi::dirs::config().join("init.zi");
    if init_path.exists() {
        for cmd in std::fs::read_to_string(init_path)?.parse::<zi::Commands>()? {
            editor.execute(cmd)?;
        }
    }

    #[cfg(unix)]
    let signal = {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate())?;
        let mut hangup = signal(SignalKind::hangup())?;
        async move {
            tokio::select! {
                _ = terminate.recv() => 143,
                _ = hangup.recv() => 129,
            }
        }
    };
    #[cfg(not(unix))]
    let signal = std::future::pending();

    let (panic_tx, panic_rx) = std::sync::mpsc::sync_channel(1);
    let panicked = Arc::new(Notify::new());
    let notify_panic = Arc::clone(&panicked);
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::sync::mpsc::TrySendError;

        match panic_tx.try_send((info.to_string(), Backtrace::capture())) {
            Ok(()) => notify_panic.notify_one(),
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => previous_hook(info),
        }
    }));

    let mut app = zi_term::App::new(term, panic_rx)?;
    app.enter()?;

    let events = EventStream::new()
        .filter_map(|ev| async { ev.map(|ev| Event::try_from(ev).ok()).transpose() });

    let client = editor.client();
    tokio::spawn(async move {
        plugins_initialized.await;
        if let Some(path) = opts.path {
            if path.exists() && path.is_dir() {
                std::env::set_current_dir(&path)?;
                client.with(|editor| editor.open_file_explorer(".")).await;
            } else {
                let mut flags = zi::OpenFlags::SPAWN_LANGUAGE_SERVICES;

                if opts.readonly {
                    flags.insert(zi::OpenFlags::READONLY);
                }

                client.with(move |editor| editor.open(path, flags)).await?.await?;
            }
        }
        Ok::<_, zi::Error>(())
    });

    tokio::task::yield_now().await;

    let mut status = ExitCode::SUCCESS;
    let shutdown = async {
        status = ExitCode::from(tokio::select! {
            () = panicked.notified() => 101,
            code = signal => code,
        });
    };
    app.run(&mut editor, events, tasks, shutdown).await?;

    Ok(status)
}
