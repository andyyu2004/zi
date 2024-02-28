use std::backtrace::Backtrace;
use std::io;
use std::pin::pin;
use std::sync::mpsc::Receiver;

use crossterm::cursor::SetCursorStyle;
use crossterm::event::DisableMouseCapture;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::{cursor, execute, terminal};
use futures_util::{Stream, StreamExt};
use tokio::select;
use tui::{Backend, Frame, Terminal};
use zi::input::Event;
use zi::Editor;

pub struct App<B: Backend + io::Write> {
    editor: Editor,
    term: Terminal<B>,
    panic_rx: Receiver<(String, Backtrace)>,
}

impl<B: Backend + io::Write> App<B> {
    pub fn new(
        term: Terminal<B>,
        editor: Editor,
        panic_rx: Receiver<(String, Backtrace)>,
    ) -> io::Result<Self> {
        Ok(Self { term, editor, panic_rx })
    }

    pub fn enter(&mut self) -> io::Result<()> {
        execute!(self.term.backend_mut(), EnterAlternateScreen, DisableMouseCapture)?;
        terminal::enable_raw_mode()?;
        Ok(())
    }

    pub async fn run(
        &mut self,
        mut events: impl Stream<Item = io::Result<Event>>,
        tasks: zi::Callbacks,
    ) -> io::Result<()> {
        self.render()?;

        let mut tasks = tasks.buffer_unordered(16);

        let mut events = pin!(events);
        loop {
            select! {
                f = tasks.select_next_some() => match f {
                    Ok(f) => if let Err(err) = f(&mut self.editor) {
                        tracing::error!("task callback failed: {:?}", err);
                    }
                    // TODO show error somewhere
                    Err(err) => tracing::error!("task failed: {:?}", err),
                },
                Some(event) = events.next() => self.on_event(event?),
            }

            if self.editor.should_quit() {
                break;
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

        self.editor.cleanup().await;

        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    fn render(&mut self) -> io::Result<()> {
        self.term.draw(|frame| render(&self.editor, frame))?;
        Ok(())
    }

    fn on_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.editor.handle_key_event(key),
            Event::Resize(_, _) => {}
        }
    }
}

pub fn render(editor: &Editor, frame: &mut Frame<'_>) {
    let area = frame.size();
    editor.render(area, frame.buffer_mut());

    let (view, buf) = editor.active();
    let (x, y) = view.cursor_viewport_coords(buf);
    // + 1 for a blank space between line number and text
    const LINE_NR_WIDTH: usize = 4;
    // FIXME this const is duplicated
    frame.set_cursor(((LINE_NR_WIDTH as u32) + x + 1) as u16, y as u16);
}

impl<W: Backend + io::Write> Drop for App<W> {
    fn drop(&mut self) {
        _ = execute!(self.term.backend_mut(), crossterm::terminal::LeaveAlternateScreen);
        _ = terminal::disable_raw_mode();
        if let Ok((panic, backtrace)) = self.panic_rx.try_recv() {
            eprintln!("{panic}");
            eprintln!("{backtrace}");
        }
    }
}
