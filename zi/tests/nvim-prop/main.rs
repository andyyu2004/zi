use std::sync::OnceLock;

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

macro_rules! t {
    ( $text:tt, $inputs:tt, $name:ident $(, $filter:expr)?) => {
        ::proptest::proptest! {
            #[test]
            fn $name(text in $text, inputs in $inputs) {
                $( $filter(&inputs)?; )?

                run(text, inputs)
            }
        }
    };
}

// Too many annoying cases here for word motions :)
// Also the default behaviour is not great anyway so leave this for later.
// We probably want to implement something closer to vim-wordmotion by default
// t!(r"(?s)[ -~]*", "[wbjk]+", nvim_word_motions);

t!(r"(?s)[ a-z]*", "<ESC>", nvim_test);

t!(r"(?s)[ -~]*", "[WBjk]+", nvim_token_motions);
t!(r"(?s)[ -~]*", "[dWBjk]+", nvim_delete_operator);
t!(r"(?s)[ -~]*", "([dWBjk]|(<ESC>))+u", nvim_undo_delete);
// t!(r"(?s)[ -~]*", "([abucdWBjk]|(<ESC>))+", nvim_undo);

#[track_caller]
fn run(text: String, inputs: String) {
    rt().block_on(async move {
        let fixture = Fixture::new([TestCase::new(text, inputs.as_str())]);
        let nvim = nvim(&fixture).await;
        fixture.nvim_vs_zi_with(nvim).await.unwrap();
    })
}
