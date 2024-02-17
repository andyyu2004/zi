mod event;

use std::io;

use crossterm::event::{DisableMouseCapture, EventStream};
use crossterm::terminal::EnterAlternateScreen;
use crossterm::{execute, terminal};
use event::*;
use futures_util::{Stream, StreamExt};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;
use tokio::select;
use zi::Editor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        let mut events = std::pin::pin!(events);
        loop {
            if self.editor.quit {
                break;
            }

            select! {
                Some(event) = events.next() => self.on_event(event?).await,
            }

            self.render();
        }

        Ok(())
    }

    fn render(&self) {}

    async fn on_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.editor.on_key(key),
            Event::Resize(_, _) => {}
        }
    }
}

// #[macro_export]
// macro_rules! ctrl {
//     ($code:expr) => {
//         key!(ctrl $code)
//     };
// }
//
// #[macro_export]
// macro_rules! key {
//     ($code:expr) => {
//         Event::Key(KeyEvent { code: KeyCode::Char($code), modifiers: KeyModifiers::empty() })
//     };
//     (ctrl $code:expr) => {
//         Event::Key(KeyEvent { code: KeyCode::Char($code), modifiers: KeyModifiers::CONTROL })
//     };
//     (alt $code:expr) => {
//         Event::Key(KeyEvent { code: KeyCode::Char($code), modifiers: KeyModifiers::ALT })
//     };
//     (shift $code:expr) => {
//         Event::Key(KeyEvent { code: KeyCode::Char($code), modifiers: KeyModifiers::SHIFT })
//     };
// }

impl<W: Backend + io::Write> Drop for App<W> {
    fn drop(&mut self) {
        _ = execute!(self.term.backend_mut(), crossterm::terminal::LeaveAlternateScreen);
        _ = terminal::disable_raw_mode();
    }
}
