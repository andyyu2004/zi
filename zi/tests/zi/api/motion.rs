use crate::new;

#[tokio::test]
async fn motion_scrolls_view() {
    let cx = new("0\n1\n2\n3\n4\n").with_size((10, 4)).await;
    cx.with(|editor| {
        editor.set_cursor(zi::Active, (0, 0));
        for i in 0..5usize {
            assert_eq!(editor.cursor_line(), format!("{i}"));
            assert_eq!(editor.view(zi::Active).offset(), zi::Offset::new(i.saturating_sub(1), 0));
            editor.motion(zi::Active, zi::motion::NextWord);
        }
    })
    .await;
    cx.cleanup().await;
}
