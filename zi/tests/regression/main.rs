use std::path::Path;

use arbitrary::{Arbitrary, Unstructured};
use datatest_stable::{Result, harness};
use zi::input::{KeyCode, KeyEvent, KeySequence};

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

harness!(test, "tests/regression/fuzz", r"^.*/*");

#[tokio::main(flavor = "current_thread")]
async fn test(path: &Path) -> Result<()> {
    let bytes = std::fs::read(path)?;
    let u = Unstructured::new(&bytes);
    let seq = <KeySequence as Arbitrary>::arbitrary_take_rest(u)?;
    run(seq).await
}

async fn run(seq: KeySequence) -> Result<()> {
    let (width, height) = (24, 10);
    let mut frame = tui::TestFrame::new(width, height);
    let (mut editor, tasks) = zi::Editor::new(zi::DummyBackend, zi::Size::new(width, height));

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

    editor
        .run(inputs, tasks, |editor| {
            // don't use `term.draw()` as it's very slow
            editor.render(&mut frame);
            Ok(())
        })
        .await?;

    Ok(())
}
