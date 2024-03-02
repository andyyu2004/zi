use crate::input::{KeyEvent, KeySequence};

#[test]
fn parse_key_event() {
    for s in [
        "a", "b", "<C-a>", "<C-S-d>", "A", "<BS>", "<C-BS>", "<c-cr>", "<c-CR>", "<C-a>",
        "<C-S-a>", "<C-S-A>", "<C-S-BS>", "<C-S-CR>",
    ] {
        let parsed = s.parse::<KeyEvent>().unwrap_or_else(|err| {
            panic!("case: `{s}` should parse but failed to parse with error: `{err:?}`")
        });
        let x = parsed.to_string();
        assert_eq!(parsed, x.parse::<KeyEvent>().unwrap(), "case: raw=`{s}` to_string=`{x}`",);
    }
}

#[test]
fn parse_key_event_fail() {
    for s in ["C-a", "C-S-d"] {
        let parsed = s.parse::<KeyEvent>();
        assert!(
            parsed.is_err(),
            "case: `{s}` should fail to parse but parsed as `{:?}`",
            parsed.unwrap()
        )
    }
}

#[test]
fn parse_key_sequence() {
    for s in ["<C-r>s", "abcd<C-r>s", "<C-r><C-r>s", "<C-w>j", "<C-w>'", "<C-w>\""] {
        let parsed = s.parse::<KeySequence>().unwrap_or_else(|err| {
            panic!("case: `{s}` should parse but failed to parse with error: `{err:?}`")
        });
        let x = parsed.to_string();
        assert_eq!(parsed, x.parse::<KeySequence>().unwrap(), "case: raw=`{s}` to_string=`{x}`",);
    }
}
