mod event;

use std::io;
use std::path::PathBuf;

use clap::Parser;
use crossterm::event::{DisableMouseCapture, EventStream};
use crossterm::terminal::EnterAlternateScreen;
use crossterm::{execute, terminal};
use event::*;
use futures_util::{Stream, StreamExt};
use tokio::select;
use tracing_subscriber::EnvFilter;
use zi::Editor;
use zi_tui::{Backend, CrosstermBackend, Terminal};

#[derive(Parser)]
struct Opts {
    #[clap(long)]
    log: Option<PathBuf>,
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
    let editor = zi::Editor::default();
    let term = Terminal::new(CrosstermBackend::new(stdout))?;
    let mut app = App::new(term, editor)?;
    app.enter()?;

    let events =
        EventStream::new().filter_map(|ev| async { ev.map(Event::from_crossterm).transpose() });
    app.run(events).await?;

    Ok(())
}

struct App<B: Backend + io::Write> {
    editor: Editor,
    term: Terminal<B>,
}

impl<B: Backend + io::Write> App<B> {
    fn new(term: Terminal<B>, editor: Editor) -> io::Result<Self> {
        Ok(Self { term, editor })
    }

    fn enter(&mut self) -> io::Result<()> {
        execute!(self.term.backend_mut(), EnterAlternateScreen, DisableMouseCapture)?;
        terminal::enable_raw_mode()?;
        Ok(())
    }

    async fn run(&mut self, mut events: impl Stream<Item = io::Result<Event>>) -> io::Result<()> {
        self.render()?;

        let mut events = std::pin::pin!(events);
        loop {
            if self.editor.quit {
                break;
            }

            select! {
                Some(event) = events.next() => self.on_event(event?).await,
            }

            self.render()?;
        }

        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    fn render(&mut self) -> io::Result<()> {
        let (view, buf) = self.editor.active();
        let rect = self.term.size()?;
        tracing::debug!(?rect, "rendering");
        let text = buf.text();
        self.term.draw(|frame| {
            text.lines().for_each(|line| {});
            frame.set_cursor(0, 0);
        })?;
        Ok(())
    }

    async fn on_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.editor.on_key(key),
            Event::Resize(_, _) => {}
        }
    }
}

impl<W: Backend + io::Write> Drop for App<W> {
    fn drop(&mut self) {
        _ = execute!(self.term.backend_mut(), crossterm::terminal::LeaveAlternateScreen);
        _ = terminal::disable_raw_mode();
    }
}
