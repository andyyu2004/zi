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
    ( $text:expr, $inputs:tt, $name:ident $(, $filter:expr)?) => {
        ::proptest::proptest! {
            #[test]
            fn $name(inputs in $inputs, text in $text) {
                $( $filter(&inputs)?; )?

                // `:help cw`
                // stupid special case in neovim that I can't find a good workaround for.
                // Also don't want to implement it as it's not a good default behaviour.
                proptest::prop_assume!(!inputs.contains("cw") && !inputs.contains("cW"));

                // `cb` and `db` have some interesting (undocumented?) behaviour (repro with text="a\nb" input="wcb") where the first newline is not removed.
                // Not sure how to go about matching this behaviour so skip these cases for now
                proptest::prop_assume!(!inputs.contains("cb") && !inputs.contains("cB"));

                // avoid lines with only spaces, as often formatters will clear trailing whitespaces anyway
                proptest::prop_assume!(!text.contains("\n \n"));

                run(text, &inputs)
            }
        }
    };
}

// Too many annoying cases here for word motions :)
// Also the default behaviour is not great anyway so leave this for later.
// We probably want to implement something closer to vim-wordmotion by default
// t!(r"(?s)[ -~]*", "[wbjk]+", nvim_word_motions);

const I: &str = r"(?s)[A-z][ -~\n]*[A-z]";

// TODO add hjkl motions to tests once implemented as motions
t!(I, "<ESC>", nvim_test);
t!(I, "[WBjk]+", nvim_token_motions);
t!(I, "[dWB]+", nvim_delete_word);
// t!(INPUT, "[cWBjk]+<ESC>", nvim_change_operator);
t!(I, "d([WB]|(<ESC>))+u<ESC>", nvim_undo_delete_word);
t!(I, "([ucdWB]|(<ESC>))+<ESC>", nvim_undo);

#[test]
fn scratch() {
    // useful to test a particular case
    // run("A\n\na", "WWWdWdBdW");
    // run("a b\nc", "jdb");
    // run("ab\nc", "jdb");
    // run("A0\naA", "WWWWdddB");
    // run("\n A", "dW");
    run("\n A", "dd");
    run("a\na", "WdBu");
}

#[track_caller]
fn run(text: impl Into<String>, inputs: &str) {
    rt().block_on(async move {
        let fixture = Fixture::new([TestCase::new(text, inputs)]);
        let nvim = nvim(&fixture).await;
        fixture.nvim_vs_zi_with(nvim).await.unwrap();
    })
}
