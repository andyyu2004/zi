use zi::Mode;

use super::*;

#[test]
fn cmd_mode_backspace() {
    let mut editor = new("");
    editor.input(":").unwrap();
    assert_eq!(editor.mode(), Mode::Command);
    assert_eq!(editor.command_buffer(), ":");

    editor.input("<BS>").unwrap();
    assert_eq!(editor.command_buffer(), "");
    assert_eq!(editor.mode(), Mode::Normal, "deleting the colon should return to normal mode");
}
