#![feature(panic_update_hook)]

use std::backtrace::Backtrace;
use std::io;
use std::path::PathBuf;

use clap::Parser;
use crossterm::event::EventStream;
use futures_util::StreamExt;
use tracing_subscriber::EnvFilter;
use tui::{CrosstermBackend, Terminal};
use zi::input::Event;

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
    let term = Terminal::new(CrosstermBackend::new(stdout))?;
    let (mut editor, callbacks, notify_redraw) = zi::Editor::new(term.size()?);
    if let Some(path) = opts.path {
        if path.exists() && path.is_dir() {
            std::env::set_current_dir(&path)?;
            editor.open_file_explorer(".");
        } else {
            editor.open_active(path)?;
        }
    }

    let (panic_tx, panic_rx) = std::sync::mpsc::sync_channel(1);
    std::panic::update_hook(move |prev, info| {
        let backtrace = Backtrace::capture();
        let _ = panic_tx.send((info.to_string(), backtrace));
        prev(info);
    });

    let mut app = zi_term::App::new(term, editor, panic_rx)?;
    app.enter()?;

    let events = EventStream::new()
        .filter_map(|ev| async { ev.map(|ev| Event::try_from(ev).ok()).transpose() });
    app.run(events, callbacks, notify_redraw).await?;

    Ok(())
}
