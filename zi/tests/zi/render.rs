mod insert;
mod split;

use tui::backend::{Backend as _, TestBackend};
use tui::Terminal;
use unicode_width::UnicodeWidthStr;

pub fn new_with_snapshot(
    size: zi::Size,
    content: &str,
) -> (zi::Editor, impl FnMut(&mut zi::Editor, expect_test::Expect)) {
    let (mut editor, _tasks) = zi::Editor::new(size);
    editor.set_mode(zi::Mode::Insert);
    for c in content.chars() {
        editor.insert_char_at_cursor(c);
    }

    let mut term = Terminal::new(TestBackend::new(size.width, size.height)).unwrap();
    (editor, move |editor, expect| {
        term.draw(|f| editor.render(f)).unwrap();
        expect.assert_eq(&render(term.backend_mut()))
    })
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
