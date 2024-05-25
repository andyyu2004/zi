use crate::new_cx_with_size;

#[tokio::test]
async fn motion_scrolls_view() {
    let height = 4;
    let cx = new_cx_with_size(zi::Size::new(10, height), "0\n1\n2\n3\n4\n").await;
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
