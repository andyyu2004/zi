#![feature(panic_update_hook)]

use std::backtrace::Backtrace;
use std::io;
use std::path::PathBuf;

use clap::Parser;
use crossterm::event::EventStream;
use futures_util::StreamExt;
use tracing_subscriber::EnvFilter;
use tui::{CrosstermBackend, Terminal};

#[derive(Parser)]
struct Opts {
    #[clap(long)]
    log: Option<PathBuf>,
    path: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    if let Some(log) = opts.log {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(log)?;
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .with_env_filter(EnvFilter::from_env("ZI_LOG"))
            .init();
    }

    let stdout = io::stdout().lock();
    let (mut editor, callbacks) = zi::Editor::new();
    if let Some(path) = opts.path {
        editor.open(path)?;
    }
    let term = Terminal::new(CrosstermBackend::new(stdout))?;

    let (panic_tx, panic_rx) = std::sync::mpsc::sync_channel(1);
    std::panic::update_hook(move |prev, info| {
        let backtrace = Backtrace::capture();
        let _ = panic_tx.send((info.to_string(), backtrace));
        prev(info);
    });

    let mut app = zi_term::App::new(term, editor, panic_rx)?;
    app.enter()?;

    let events = EventStream::new()
        .filter_map(|ev| async { ev.map(zi_term::Event::from_crossterm).transpose() });
    app.run(events, callbacks).await?;

    Ok(())
}
