use zi::Editor;

use crate::new;

#[tokio::test]
async fn buffer_settings() {
    let cx = new("").await;
    cx.with(|editor| {
        let config = editor.buffer(zi::Active).settings();
        assert_eq!(config.tab_width, 4);
        config.tab_width.write(8);
        assert_eq!(config.tab_width, 8);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn buffer_set_command() {
    fn buf(editor: &Editor) -> &zi::buffer::Settings {
        editor.buffer(zi::Active).settings()
    }

    fn view(editor: &Editor) -> &zi::view::Settings {
        editor.view(zi::Active).settings()
    }

    let cx = new("").await;
    cx.with(|editor| {
        // :set tabwidth
        assert_eq!(buf(editor).tab_width, 4);

        editor.execute("set tabwidth 8").unwrap();
        assert_eq!(buf(editor).tab_width, 8);

        editor.execute("set ts 4").unwrap();
        assert_eq!(buf(editor).tab_width, 4);

        editor.execute("set tabstop 6").unwrap();
        assert_eq!(buf(editor).tab_width, 6);

        // :set numberwidth
        assert_eq!(view(editor).line_number_width, 4);

        editor.execute("set numberwidth 6").unwrap();
        assert_eq!(view(editor).line_number_width, 6);

        editor.execute("set nuw 3").unwrap();
        assert_eq!(view(editor).line_number_width, 3);
    })
    .await;
    cx.cleanup().await;
}
