use std::sync::OnceLock;

use tokio::sync::OnceCell;
use zi_nvim::{CompareFlags, Fixture, Nvim, TestCase, spawn};

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

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
    ( $text:expr, $inputs:tt, $name:ident) => {
        t!($text, $inputs, $name, ::zi_nvim::CompareFlags::empty());
    };
    ( $text:expr, $inputs:tt, $name:ident, $flags:expr) => {
        ::proptest::proptest! {
            #[test]
            fn $name(inputs in $inputs, text in $text) {
                // `:help cw`
                // stupid special case in neovim that I can't find a good workaround for.
                // Also don't want to implement it as it's not a good default behaviour.
                proptest::prop_assume!(!inputs.contains("cw") && !inputs.contains("cW"));

                // `cb` and `db` have some interesting (undocumented?) behaviour (repro with text="a\nb" input="wcb") where the first newline is not removed.
                // Not sure how to go about matching this behaviour so skip these cases for now
                proptest::prop_assume!(!inputs.contains("cb") && !inputs.contains("cB"));

                run(text, &inputs, $flags)
            }
        }
    };
}

// Too many annoying cases here for word motions :)
// Also the default behaviour is not great anyway so leave this for later.
// We probably want to implement something closer to vim-wordmotion by default
// t!(r"(?s)[ -~]*", "[wbjk]+", nvim_word_motions);

const I: &str = r"(?s)[A-z][ -~\n]*[A-z]";

t!(I, "<ESC>", nvim_test);
t!(I, "[WBhjkl]+", nvim_token_motions);
t!(I, "[dWBhjkl]+", nvim_delete_operator);
// t!(INPUT, "[cWBjk]+<ESC>", nvim_change_operator);
// TODO add hjkl motions to undo tests
t!(I, "d([uWB]|(<ESC>))+<ESC>", nvim_undo_delete_word);
t!(I, "([ucdWB]|(<ESC>))+<ESC>", nvim_undo);
t!(I, "[CDWBhjkl]", nvim_big_cd);

/// Useful to test a particular case
#[test]
fn scratch() {
    #[allow(unused)]
    fn test(text: &str, inputs: &str) {
        run(text, inputs, CompareFlags::empty())
    }

    test("AA", "ccuu<ESC>");
    // test("A\n  a", "Wlkdkj");
}

#[track_caller]
fn run(text: impl Into<String>, inputs: &str, flags: CompareFlags) {
    rt().block_on(async move {
        let fixture = Fixture::new([TestCase::new(text, inputs)]);
        let nvim = nvim(&fixture).await;
        fixture.nvim_vs_zi_with(nvim, flags).await.unwrap();
    })
}
