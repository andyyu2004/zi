use super::new;

#[test]
fn search() {
    // TODO what if we switch active buffers and a match is out of range etc
    let mut editor = new(r#"abc
def
abc
"#);
    assert_eq!(editor.matches().len(), 0);

    assert!(editor.search("abc").map(|m| m.range()).eq([0..3, 8..11]));
    editor.goto_next_match();
}
