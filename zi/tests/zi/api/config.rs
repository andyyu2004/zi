use zi::Editor;

use super::new;

#[test]
fn buffer_settings() {
    let editor = new("");

    let config = editor.buffer(zi::Active).config();
    assert_eq!(config.tab_width, 4);
    config.tab_width.write(8);
    assert_eq!(config.tab_width, 8);
}

#[test]
fn buffer_set_command() {
    let mut editor = new("");

    fn buf(editor: &Editor) -> &zi::buffer::Config {
        editor.buffer(zi::Active).config()
    }

    fn view(editor: &Editor) -> &zi::view::Config {
        editor.view(zi::Active).config()
    }

    // :set tabwidth
    assert_eq!(buf(&editor).tab_width, 4);

    editor.execute("set tabwidth 8").unwrap();
    assert_eq!(buf(&editor).tab_width, 8);

    editor.execute("set ts 4").unwrap();
    assert_eq!(buf(&editor).tab_width, 4);

    editor.execute("set tabstop 6").unwrap();
    assert_eq!(buf(&editor).tab_width, 6);

    // :set numberwidth
    assert_eq!(view(&editor).line_number_width, 4);

    editor.execute("set numberwidth 6").unwrap();
    assert_eq!(view(&editor).line_number_width, 6);

    editor.execute("set nuw 3").unwrap();
    assert_eq!(view(&editor).line_number_width, 3);
}
