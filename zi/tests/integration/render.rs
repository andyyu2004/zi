mod view;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

pub fn new_with_snapshot(
    size: zi::Size,
    content: &str,
) -> (zi::Editor, impl FnMut(&mut zi::Editor, expect_test::Expect)) {
    let (mut editor, ..) = zi::Editor::new(size);
    for c in content.chars() {
        editor.insert_char(c);
    }

    let mut term = Terminal::new(TestBackend::new(size.width, size.height)).unwrap();
    (editor, move |editor, expect| {
        term.draw(|f| editor.render(f)).unwrap();
        expect.assert_eq(&term.backend().to_string())
    })
}
