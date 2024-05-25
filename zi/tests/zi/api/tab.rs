use zi::buffer::IndentSettings;

use crate::new;

#[tokio::test]
async fn tab_insert() {
    let cx = new("").await;
    cx.with(|editor| {
        assert_eq!(*editor.buffer(zi::Active).settings().indent.read(), IndentSettings::Spaces(4));

        editor.set_mode(zi::Mode::Insert);
        assert_eq!(editor.cursor_line(), "");
        editor.tab(zi::Active);
        assert_eq!(editor.cursor_line(), "    ");
        editor.tab(zi::Active);
        assert_eq!(editor.cursor_line(), "        ");
    })
    .await;

    cx.cleanup().await;
}
