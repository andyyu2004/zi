use super::new;

#[test]
fn config() {
    let editor = new("");

    let config = editor.buffer(zi::Active).config();
    assert_eq!(config.tab_width.read(), 4);
    config.tab_width.write(8);
    assert_eq!(config.tab_width.read(), 8);
}
