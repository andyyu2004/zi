use std::backtrace::Backtrace;
use std::io;
use std::sync::mpsc::Receiver;

use crossterm::cursor::SetCursorStyle;
use crossterm::event::DisableMouseCapture;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::{cursor, execute, terminal};
use futures_util::Stream;
use mimalloc::MiMalloc;
use tui::{Backend, Terminal};
use zi::Editor;
use zi::input::Event;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub struct App<B: Backend + io::Write> {
    term: Terminal<B>,
    panic_rx: Receiver<(String, Backtrace)>,
}

impl<B: Backend + io::Write> App<B> {
    pub fn new(term: Terminal<B>, panic_rx: Receiver<(String, Backtrace)>) -> io::Result<Self> {
        Ok(Self { term, panic_rx })
    }

    pub fn enter(&mut self) -> io::Result<()> {
        execute!(self.term.backend_mut(), EnterAlternateScreen, DisableMouseCapture)?;
        terminal::enable_raw_mode()?;
        Ok(())
    }

    pub async fn run(
        mut self,
        editor: &mut Editor,
        events: impl Stream<Item = io::Result<Event>>,
        tasks: zi::Tasks,
    ) -> io::Result<()> {
        editor
            .run(events, tasks, |editor| {
                // Cursor styling isn't really exposed through the ratatui API, so we just hack it here.
                // Looks much less janky if we set the cursor before rendering.
                let style = match editor.mode() {
                    zi::Mode::Normal | zi::Mode::Visual => SetCursorStyle::SteadyBlock,
                    zi::Mode::Insert | zi::Mode::Command => SetCursorStyle::SteadyBar,
                    zi::Mode::OperatorPending(..) | zi::Mode::ReplacePending => {
                        SetCursorStyle::SteadyUnderScore
                    }
                };
                execute!(self.term.backend_mut(), cursor::Show, style)?;

                self.render(editor)
            })
            .await
    }

    #[tracing::instrument(skip_all, level = "debug")]
    fn render(&mut self, editor: &mut Editor) -> io::Result<()> {
        self.term.draw(|frame| editor.render(frame))?;
        Ok(())
    }
}

impl<W: Backend + io::Write> Drop for App<W> {
    fn drop(&mut self) {
        _ = execute!(self.term.backend_mut(), crossterm::terminal::LeaveAlternateScreen);
        _ = terminal::disable_raw_mode();

        if let Ok((panic, backtrace)) = self.panic_rx.try_recv() {
            use std::io::Write as _;
            let mut stderr = io::stderr().lock();
            let _ = writeln!(stderr, "{panic}");
            let _ = writeln!(stderr, "{backtrace}");
        }
    }
}
