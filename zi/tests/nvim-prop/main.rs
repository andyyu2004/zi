use std::sync::OnceLock;

use test_strategy::proptest;
use tokio::sync::OnceCell;
use zi_nvim::{spawn, Fixture, Nvim, TestCase};

fn rt() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap())
}

// Use a single nvim instance for all tests (and by extension a tokio runtime) as spawning is slow.
async fn nvim(fixture: &Fixture) -> &'static Nvim {
    let size = fixture.size();
    static NVIM: OnceCell<zi_nvim::Nvim> = OnceCell::const_new();
    NVIM.get_or_init(|| spawn(size.height, size.width)).await
}

// Too many annoying cases here :)
// Also the default behaviour is not great anyway so leave this for later.
// #[proptest]
// fn nvim_word_motions(
//     #[strategy(r"(?s)[ A-Za-z-_]+")] text: String,
//     #[strategy("[wbjk]+")] inputs: String,
// ) {
//      run(TestCase::new(text, inputs.as_str()))
// }

#[allow(non_snake_case)]
#[proptest]
fn nvim_token_motions(
    #[strategy(r"(?s)[ A-Za-z0-9-_]+")] text: String,
    #[strategy("[WBjk]+")] inputs: String,
) {
    run(TestCase::new(text, inputs.as_str()))
}

fn run(case: TestCase) {
    rt().block_on(async move {
        let fixture = Fixture::new([case]);
        let nvim = nvim(&fixture).await;
        fixture.nvim_vs_zi_with(nvim).await.unwrap();
    })
}
