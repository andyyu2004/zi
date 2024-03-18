#![no_main]

use futures_util::stream;
use libfuzzer_sys::fuzz_target;
use zi::input::{KeyCode, KeyEvent, KeySequence};
use zi_tui::backend::TestBackend;
use zi_tui::Terminal;

fuzz_target!(|inputs: Vec<zi::input::KeyEvent>| {
    eprintln!("{}", KeySequence::from_iter(inputs.iter().cloned()));

    let (width, height) = (80, 24);
    let (editor, tasks) = zi::Editor::new(zi::Size::new(width, height));

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

    // Keep inputting a quit sequence until we're done otherwise we will get stuck on one iteration.
    let inputs = stream::iter(
        inputs.into_iter().chain(quit_sequence.cycle()).map(zi::input::Event::Key).map(Ok),
    );

    tokio::runtime::Runtime::new().unwrap().block_on(async move {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        editor
            .run(inputs, tasks, |editor| {
                term.draw(|frame| editor.render(frame)).unwrap();
                Ok(())
            })
            .await
            .unwrap();
    });
});
