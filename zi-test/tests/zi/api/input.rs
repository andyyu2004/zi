use crate::new;

#[tokio::test]
async fn bracketed_paste_is_literal_and_repeatable() {
    let cx = new("").await;
    cx.with(|editor| {
        editor.input("if").unwrap();
        editor.handle_input(zi::input::Event::Paste("d{\r\n\tλ:quit\r\nx".into()));
        assert_eq!(editor.text(zi::Active).to_string(), "fd{\n\tλ:quit\nx\n");
        assert_eq!(editor.cursor(zi::Active), (2, 1));
        assert_eq!(editor.mode(), zi::Mode::Insert);

        editor.input("<Esc>u").unwrap();
        assert!(editor.text(zi::Active).is_empty());
        editor.input(".").unwrap();
        assert_eq!(editor.text(zi::Active).to_string(), "fd{\n\tλ:quit\nx\n");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn bracketed_paste_replaces_a_visual_line() {
    let cx = new("first\nsecond\nthird").await;
    cx.with(|editor| {
        editor.input("kV").unwrap();
        editor.handle_input(zi::input::Event::Paste("replacement".into()));
        assert_eq!(editor.text(zi::Active).to_string(), "first\nreplacement\nthird\n");
        assert_eq!(editor.mode(), zi::Mode::Normal);
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn bracketed_paste_preserves_trailing_whitespace_after_escape() {
    let cx = new("seed").await;
    cx.with(|editor| {
        editor.input("A").unwrap();
        editor.handle_input(zi::input::Event::Paste("end  \n\t ".into()));
        editor.input("<Esc>").unwrap();
        assert_eq!(editor.text(zi::Active).to_string(), "seedend  \n\t \n");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn bracketed_paste_repeats_with_a_counted_insert() {
    let cx = new("").await;
    cx.with(|editor| {
        editor.input("2i").unwrap();
        editor.handle_input(zi::input::Event::Paste("x  ".into()));
        editor.input("<Esc>").unwrap();
        assert_eq!(editor.text(zi::Active).to_string(), "x  x  \n");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn escape_clears_untouched_autoindent() {
    let cx = new("{").await;
    cx.with(|editor| {
        editor.input("A<CR>").unwrap();
        assert_eq!(editor.cursor_line(), "    ");
        editor.input("<Esc>").unwrap();
        assert_eq!(editor.text(zi::Active).to_string(), "{\n\n");
    })
    .await;
    cx.cleanup().await;
}
