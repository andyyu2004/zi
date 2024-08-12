#![feature(panic_update_hook)]

use std::backtrace::Backtrace;
use std::io;
use std::path::PathBuf;

use clap::Parser;
use crossterm::event::EventStream;
use futures_util::StreamExt;
use tracing_subscriber::EnvFilter;
use tui::backend::CrosstermBackend;
use tui::Terminal;
use zi::input::Event;
use zi::{filetype, LanguageConfig};
use zi_lsp::LanguageServerConfig;

#[derive(Parser)]
struct Opts {
    #[clap(long)]
    log: Option<PathBuf>,
    path: Option<PathBuf>,
    #[clap(long)]
    readonly: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut opts = Opts::parse();

    const ZI_LOG: &str = "ZI_LOG";

    if std::env::var(ZI_LOG).is_ok() && opts.log.is_none() {
        // If the user has set the ZI_LOG environment variable, but not specified a log file, default to /tmp/zi.log
        opts.log = Some(PathBuf::from("/tmp/zi.log"));
    }

    if let Some(log) = opts.log {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)?;
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .with_env_filter(EnvFilter::from_env(ZI_LOG))
            .init();
    }

    let stdout = io::stdout().lock();
    let term = Terminal::new(CrosstermBackend::new(stdout))?;
    let (mut editor, tasks) = zi::Editor::new(zi_wasm::WasmBackend::default(), term.size()?);

    configure(&mut editor);

    let init_path = zi::dirs::config().join("init.zi");
    if init_path.exists() {
        for cmd in std::fs::read_to_string(init_path)?.parse::<zi::Commands>()? {
            editor.execute(cmd)?;
        }
    }

    let (panic_tx, panic_rx) = std::sync::mpsc::sync_channel(1);
    std::panic::update_hook(move |prev, info| {
        let backtrace = Backtrace::capture();
        let _ = panic_tx.send((info.to_string(), backtrace));
        prev(info);
    });

    let mut app = zi_term::App::new(term, panic_rx)?;
    app.enter()?;

    let events = EventStream::new()
        .filter_map(|ev| async { ev.map(|ev| Event::try_from(ev).ok()).transpose() });

    let client = editor.client();
    tokio::spawn(async move {
        if let Some(path) = opts.path {
            if path.exists() && path.is_dir() {
                std::env::set_current_dir(&path)?;
                client.with(|editor| editor.open_file_explorer(".")).await;
            } else {
                let mut flags = zi::OpenFlags::SPAWN_LANGUAGE_SERVICES;

                if opts.readonly {
                    flags.insert(zi::OpenFlags::READONLY);
                }

                client
                    .with(move |editor| editor.open(path, flags))
                    .await?
                    .await?;
            }
        }
        Ok::<_, zi::Error>(())
    });

    tokio::task::yield_now().await;

    app.run(&mut editor, events, tasks).await?;

    Ok(())
}

fn configure(editor: &mut zi::Editor) {
    editor
        .language_config_mut()
        .add_language(
            filetype!(rust),
            LanguageConfig::new(["rust-analyzer".into()]),
        )
        .add_language(filetype!(text), LanguageConfig::new([]))
        .add_language(filetype!(toml), LanguageConfig::new([]))
        .add_language(filetype!(json), LanguageConfig::new([]))
        .add_language(filetype!(haskell), LanguageConfig::new(["hls".into()]))
        .add_language(filetype!(go), LanguageConfig::new(["gopls".into()]))
        .add_language(filetype!(gqlt), LanguageConfig::new(["gqlt".into()]))
        .add_language(filetype!(c), LanguageConfig::new(["clangd".into()]))
        .add_language(
            filetype!(javascript),
            LanguageConfig::new(["tsserver".into()]),
        )
        .add_language(
            filetype!(typescript),
            LanguageConfig::new(["tsserver".into()]),
        )
        .add_language_service(
            "rust-analyzer",
            // LanguageServerConfig::new("ra-multiplex", []),
            LanguageServerConfig::new("rust-analyzer", []),
        )
        .add_language_service("gopls", LanguageServerConfig::new("gopls", []))
        .add_language_service("gqlt", LanguageServerConfig::new("gqlt", []))
        .add_language_service("clangd", LanguageServerConfig::new("clangd", []))
        .add_language_service(
            "hls",
            LanguageServerConfig::new("haskell-language-server-wrapper", ["--lsp".into()]),
        )
        .add_language_service("tsserver", LanguageServerConfig::new("tsserver", []));
}
