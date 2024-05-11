mod file_picker;
mod insert;
mod line_number;
mod split;

use expect_test::{expect, Expect};
use tokio::task::LocalSet;
use tui::backend::{Backend as _, TestBackend};
use tui::Terminal;
use unicode_width::UnicodeWidthStr;

pub async fn run(
    size: zi::Size,
    content: &str,
    f: impl FnOnce(&mut zi::Editor, Box<dyn FnMut(&mut zi::Editor, Expect)>) + Send + 'static,
) {
    let (mut editor, tasks) = zi::Editor::new(size);
    editor.set_mode(zi::Mode::Insert);
    editor.edit(zi::Active, &zi::Delta::insert_at(0, content));
    editor.set_cursor(zi::Active, content.len());

    let client = editor.client();
    let local = LocalSet::new();
    local.spawn_local(async move {
        editor.run(futures_util::stream::empty(), tasks, |_editor| Ok(())).await.unwrap()
    });

    let mut term = Terminal::new(TestBackend::new(size.width, size.height)).unwrap();

    local
        .run_until(client.request(move |editor| {
            let snapshot = Box::new(move |editor: &mut zi::Editor, expect: Expect| {
                term.draw(|frame| editor.render(frame)).unwrap();
                expect.assert_eq(&render(term.backend_mut()))
            });

            f(editor, snapshot);
        }))
        .await;
}

/// Copied from ratatui's `buffer_view`, but draws the cursor too.
fn render(backend: &mut TestBackend) -> String {
    use std::fmt::Write as _;

    let cursor = backend.get_cursor().unwrap();
    let buffer = backend.buffer();
    let mut view = String::with_capacity(buffer.content.len() + buffer.area.height as usize * 3);
    for (y, cells) in buffer.content.chunks(buffer.area.width as usize).enumerate() {
        let mut overwritten = vec![];
        let mut skip: usize = 0;
        view.push('"');
        for (x, c) in cells.iter().enumerate() {
            if (x as u16, y as u16) == cursor {
                view.push('|');
                continue;
            }

            if skip == 0 {
                view.push_str(c.symbol());
            } else {
                overwritten.push((x, c.symbol()));
            }
            skip = std::cmp::max(skip, c.symbol().width()).saturating_sub(1);
        }
        view.push('"');
        if !overwritten.is_empty() {
            write!(&mut view, " Hidden by multi-width symbols: {overwritten:?}").unwrap();
        }
        view.push('\n');
    }
    view
}
