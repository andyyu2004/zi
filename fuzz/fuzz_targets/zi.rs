#![no_main]

use futures_util::stream;
use libfuzzer_sys::fuzz_target;
use zi::input::{KeyCode, KeyEvent};
use zi_tui::backend::TestBackend;
use zi_tui::Terminal;

fuzz_target!(|inputs: Vec<zi::input::KeyEvent>| {
    // Keep inputting a quit sequence until we're done otherwise we will get stuck on one iteration.
    let quit_sequence = [
        KeyCode::Esc,
        KeyCode::Esc,
        KeyCode::Esc,
        KeyCode::Esc,
        KeyCode::Char(':'),
        KeyCode::Char('q'),
        KeyCode::Enter,
    ]
    .into_iter()
    .map(KeyEvent::from);

    let inputs = inputs.into_iter().chain(quit_sequence.cycle());

    let (width, height) = (80, 24);
    let (editor, tasks) = zi::Editor::new(zi::Size::new(width, height));
    let backend = TestBackend::new(width, height);
    let mut term = Terminal::new(backend).unwrap();
    let inputs = stream::iter(inputs.map(zi::input::Event::Key).map(Ok));
    futures_executor::block_on(editor.run(inputs, tasks, |editor| {
        term.draw(|frame| editor.render(frame)).unwrap();
        Ok(())
    }))
    .unwrap();
});
