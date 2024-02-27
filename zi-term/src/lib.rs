mod event;

use std::backtrace::Backtrace;
use std::io;
use std::pin::pin;
use std::sync::mpsc::Receiver;

use crossterm::cursor::SetCursorStyle;
use crossterm::event::DisableMouseCapture;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::{cursor, execute, terminal};
pub use event::*;
use futures_util::{Stream, StreamExt};
use tokio::select;
use tui::{Backend, Frame, Terminal};
use zi::{tree_sitter, Editor};

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
                Some(event) = events.next() => self.on_event(event?).await,
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

    async fn on_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.editor.on_key(key),
            Event::Resize(_, _) => {}
        }
    }
}

pub fn render(editor: &Editor, frame: &mut Frame<'_>) {
    let (view, buf) = editor.active();
    let mut cursor = tree_sitter::QueryCursor::new();
    cursor.set_match_limit(256);
    let theme = editor.theme();

    let c = |c: zi::Color| match c {
        zi::Color::Rgb(r, g, b) => tui::Color::Rgb(r, g, b),
    };

    let s = |s: zi::Style| tui::Style { fg: s.fg.map(c), bg: s.bg.map(c), ..Default::default() };

    let line = view.offset().line as usize;

    // FIXME compute highlights only for the necessary range
    let highlights = buf
        .highlights(&mut cursor)
        .skip_while(|(node, _)| node.range().end_point.row < line)
        .filter_map(|(node, id)| Some((node, s(id.style(theme)?))))
        .map(|(node, style)| {
            let range = node.range();
            let start = range.start_point;
            let end = range.end_point;
            // Need to adjust the line to be 0-based as that's what `tui::Lines` is assuming
            ((start.row - line, start.column)..(end.row - line, end.column), style)
        });

    const LINE_NR_WIDTH: usize = 4;
    let lines = tui::Lines::new(
        line,
        LINE_NR_WIDTH,
        buf.tab_width(),
        buf.text().lines_at(line),
        highlights,
    );
    let statusline = tui::Text::styled(
        format!("{}:{}:{}", buf.path().display(), view.cursor().line() + 1, view.cursor().col()),
        tui::Style::new()
            .fg(tui::Color::Rgb(0x88, 0x88, 0x88))
            .bg(tui::Color::Rgb(0x07, 0x36, 0x42)),
    );
    // guifg=#07364
    let cmdline = tui::Text::styled(
        format!("-- {} --", editor.mode()),
        tui::Style::new().fg(tui::Color::Rgb(0x88, 0x88, 0x88)),
    );

    let widget = tui::vstack(
        [tui::Constraint::Fill(1), tui::Constraint::Max(1), tui::Constraint::Max(1)],
        (lines, statusline, cmdline),
    );

    let area = frame.size();
    frame.buffer_mut().set_style(area, tui::Style::default().bg(tui::Color::Rgb(0x00, 0x2b, 0x36)));
    frame.render_widget(widget, area);

    let (x, y) = view.cursor_viewport_coords(buf);
    // + 1 for a blank space between line number and text
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
