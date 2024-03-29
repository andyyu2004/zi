use test_strategy::proptest;
use zi_nvim::{Fixture, TestCase};

#[proptest(async = "tokio")]
async fn nvim_horizontal_word_motion(
    // can't be bothered getting it right for all unicode for now
    #[strategy(r"(?s)[\sA-Za-z0-9-_]")] text: String,
    #[strategy("[wbjk]+")] inputs: String,
) {
    Fixture::new([TestCase::new(text.clone(), inputs.as_str())]).nvim_vs_zi().await.unwrap();
}
