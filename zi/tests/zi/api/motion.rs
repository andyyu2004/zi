use super::*;

#[test]
fn motion_scrolls_view() {
    let height = 2;
    let mut editor = new_with_size("0\n1\n2\n3\n4\n", zi::Size::new(10, height));

    for i in 0..5u32 {
        assert_eq!(editor.current_line(), format!("{i}"));
        assert_eq!(editor.active_view().offset(), zi::Offset::new(i.saturating_sub(1), 0));
        editor.motion(zi::motion::NextWord);
    }
}
