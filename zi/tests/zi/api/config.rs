use core::fmt;

use zi::Editor;
use zi_test::TestContext;

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

    async fn check_buf<T: PartialEq + fmt::Debug + Send + Sync + 'static>(
        cx: &TestContext,
        cmd: &'static str,
        expected: T,
        f: impl Fn(&zi::buffer::Settings) -> T + Send + Sync + 'static,
    ) {
        cx.with(move |editor| editor.execute(cmd).unwrap()).await;

        cx.with(move |editor| {
            assert_eq!(f(buf(editor)), expected);
        })
        .await;
    }

    async fn check_view<T: PartialEq + fmt::Debug + Send + Sync + 'static>(
        cx: &TestContext,
        cmd: &'static str,
        expected: T,
        f: impl Fn(&zi::view::Settings) -> T + Send + Sync + 'static,
    ) {
        cx.with(move |editor| editor.execute(cmd).unwrap()).await;

        cx.with(move |editor| {
            assert_eq!(f(view(editor)), expected);
        })
        .await;
    }

    cx.with(|editor| {
        assert_eq!(buf(editor).tab_width, 4);
    })
    .await;

    // :set tabwidth
    check_buf(&cx, "set tabwidth 8", 8, |buf| *buf.tab_width.read()).await;
    check_buf(&cx, "set ts 4", 4, |buf| *buf.tab_width.read()).await;
    check_buf(&cx, "set tabstop 2", 2, |buf| *buf.tab_width.read()).await;

    // :set numberwidth
    cx.with(|editor| assert_eq!(view(editor).line_number_width, 4)).await;
    check_view(&cx, "set numberwidth 6", 6, |view| *view.line_number_width.read()).await;
    check_view(&cx, "set nuw 3", 3, |view| *view.line_number_width.read()).await;

    cx.cleanup().await;
}
