use crate::new;

// nvim's wordmotion runs `e` once per count, so these can't be compared with nvim.

#[tokio::test]
async fn huge_count_on_word_end_motion() {
    let cx = new("a b c").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("99999999999e").unwrap();
        assert_eq!(editor.cursor(zi::Active), (0, 4));
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn huge_counts_before_and_after_an_operator_on_word_end() {
    let cx = new("a b c").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("9999999999d9999999999e").unwrap();
        assert_eq!(editor.cursor_line(), "");
    })
    .await;
    cx.cleanup().await;
}

#[tokio::test]
async fn huge_counts_on_word_motion() {
    let cx = new("a b c").await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        editor.input("9999999999d9999999999w").unwrap();
        assert_eq!(editor.cursor_line(), "");
    })
    .await;
    cx.cleanup().await;
}
