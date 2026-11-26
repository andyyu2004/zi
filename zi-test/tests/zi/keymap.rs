use zi::input::KeySequence;

use crate::new;

#[tokio::test]
async fn composite_escape() {
    async fn check(seq: &'static str, expectation: &'static str) {
        let cx = new("").await;
        cx.with(move |editor| {
            let seq = seq.parse::<KeySequence>().unwrap();
            for key in seq {
                editor.handle_input(key);
            }

            assert_eq!(editor.cursor_line(), expectation);
        })
        .await;
        cx.cleanup().await;
    }

    check("ifd", "").await;
    check("iffk", "ffk").await;
    check("iffd", "f").await;
    check("ifffx", "fffx").await;
}
