#![no_main]

use futures_util::stream;
use libfuzzer_sys::fuzz_target;
use zi_input::{KeyCode, KeyEvent};

fuzz_target!(|inputs: zi_input::KeySequence| {
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

    let inputs = inputs.into_iter().chain(quit_sequence);

    let (width, height) = (24, 10);
    let (mut editor, tasks) = zi::Editor::new(zi::DummyBackend, zi::Size::new(width, height));
    let mut frame = TestFrame { buffer: tui::Buffer::empty(tui::Rect::new(0, 0, width, height)) };
    let inputs = stream::iter(inputs.map(zi_input::Event::Key).map(Ok));

    futures_executor::block_on(editor.run(inputs, tasks, |editor| {
        // don't use `term.draw()` as it's very slow
        editor.render(&mut frame);
        Ok(())
    }))
    .unwrap();
});

struct TestFrame {
    buffer: tui::Buffer,
}

impl tui::DynFrame for TestFrame {
    fn buffer_mut(&mut self) -> &mut tui::Buffer {
        &mut self.buffer
    }

    fn set_cursor(&mut self, _x: u16, _y: u16) {}
}
