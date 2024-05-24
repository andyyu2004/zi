use super::*;

#[tokio::test]
async fn motion_scrolls_view() {
    let height = 2;
    let mut editor = new_with_size("0\n1\n2\n3\n4\n", zi::Size::new(10, height)).await;

    for i in 0..5usize {
        assert_eq!(editor.cursor_line(), format!("{i}"));
        assert_eq!(editor.view(zi::Active).offset(), zi::Offset::new(i.saturating_sub(1), 0));
        editor.motion(zi::Active, zi::motion::NextWord);
    }
}
