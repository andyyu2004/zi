use zi::input::KeySequence;

use crate::api::new;

#[tokio::test]
async fn composite_escape() {
    async fn check(seq: &str, expectation: &str) {
        let mut editor = new("").await;
        let seq = seq.parse::<KeySequence>().unwrap();
        for key in seq {
            editor.handle_input(key);
        }

        assert_eq!(editor.cursor_line(), expectation);
    }

    check("ifd", "").await;
    check("iffk", "ffk").await;
    check("iffd", "f").await;
    check("ifffx", "fffx").await;
}
