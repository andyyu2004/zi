mod event;

use std::io;
use std::path::PathBuf;

use clap::Parser;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::{DisableMouseCapture, EventStream};
use crossterm::terminal::EnterAlternateScreen;
use crossterm::{cursor, execute, terminal};
use event::*;
use futures_util::{Stream, StreamExt};
use tokio::select;
use tracing_subscriber::EnvFilter;
use tui::{Backend, CrosstermBackend, Frame, Terminal};
use zi::Editor;

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

    let content = opts.path.as_ref().map(std::fs::read_to_string).transpose()?.unwrap_or_default();
    let stdout = io::stdout().lock();
    let editor = zi::Editor::new(content);
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

            // Cursor styling isn't really exposed through the ratatui API, so we just hack it here.
            // Looks much less janky if we set the cursor before rendering.
            let style = match self.editor.mode() {
                zi::Mode::Normal => SetCursorStyle::SteadyBlock,
                zi::Mode::Insert => SetCursorStyle::SteadyBar,
            };
            execute!(self.term.backend_mut(), cursor::Show, style)?;

            self.render()?;
        }

        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    fn render(&mut self) -> io::Result<()> {
        self.term.draw(|frame| render(&self.editor, frame))?;
        Ok(())
    }

    async fn on_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.editor.on_key(key),
            Event::Resize(_, _) => {}
        }
    }
}

fn render(editor: &Editor, frame: &mut Frame) {
    let (view, buf) = editor.active();
    let el = tui::Lines::new(buf.text().lines());
    let statusline = tui::Text::raw(format!("{}", editor.mode()));
    let cmdline = tui::Text::raw("cmdline");

    let widget = tui::vstack(
        [tui::Constraint::Fill(1), tui::Constraint::Max(1), tui::Constraint::Max(1)],
        (el, statusline, cmdline),
    );

    let area = *frame.buffer_mut().area();
    frame.render_widget(widget, area);

    let cursor = view.cursor();
    let (x, y) = cursor.coords();
    frame.set_cursor(x, y);
}

impl<W: Backend + io::Write> Drop for App<W> {
    fn drop(&mut self) {
        _ = execute!(self.term.backend_mut(), crossterm::terminal::LeaveAlternateScreen);
        _ = terminal::disable_raw_mode();
    }
}
