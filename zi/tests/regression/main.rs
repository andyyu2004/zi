use std::path::Path;

use arbitrary::{Arbitrary, Unstructured};
use datatest_stable::{harness, Result};
use futures_executor::block_on;
use zi::input::{KeyCode, KeyEvent, KeyModifiers, KeySequence};

harness!(test, "tests/regression/fuzz", r"^.*/*",);

fn test(path: &Path) -> Result<()> {
    let bytes = std::fs::read(path)?;
    let u = Unstructured::new(&bytes);
    let seq = <KeySequence as Arbitrary>::arbitrary_take_rest(u)?;
    run(seq)
}

fn run(seq: KeySequence) -> Result<()> {
    eprintln!("{seq}\n{seq:#?}");

    let (width, height) = (24, 10);
    let mut frame = TestFrame { buffer: tui::Buffer::empty(tui::Rect::new(0, 0, width, height)) };
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
    let inputs = seq.into_iter().chain(quit_sequence.cycle());
    let inputs = futures_util::stream::iter(inputs.into_iter().map(zi::input::Event::Key).map(Ok));
    block_on(editor.fuzz(inputs, tasks, |editor| {
        // don't use `term.draw()` as it's very slow
        editor.render(&mut frame);
        Ok(())
    }))?;

    Ok(())
}

struct TestFrame {
    buffer: tui::Buffer,
}

impl tui::DynFrame for TestFrame {
    fn buffer_mut(&mut self) -> &mut tui::Buffer {
        &mut self.buffer
    }

    fn set_cursor(&mut self, _x: u16, _y: u16) {}
}
