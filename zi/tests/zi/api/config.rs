use zi::Editor;

use super::new;

#[tokio::test]
async fn buffer_settings() {
    let editor = new("").await;

    let config = editor.buffer(zi::Active).settings();
    assert_eq!(config.tab_width, 4);
    config.tab_width.write(8);
    assert_eq!(config.tab_width, 8);
}

#[tokio::test]
async fn buffer_set_command() {
    let mut editor = new("").await;

    fn buf(editor: &Editor) -> &zi::buffer::Settings {
        editor.buffer(zi::Active).settings()
    }

    fn view(editor: &Editor) -> &zi::view::Settings {
        editor.view(zi::Active).settings()
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
