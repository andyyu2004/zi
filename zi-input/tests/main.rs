use crossterm::event::{
    Event as CtEvent, KeyCode as CtCode, KeyEvent as CtKeyEvent, KeyEventKind,
    KeyModifiers as CtMods,
};
use zi_input::{Event, KeyCode, KeyEvent, KeyModifiers, KeySequence};

fn convert(code: CtCode, mods: CtMods, kind: KeyEventKind) -> Result<Event, ()> {
    Event::try_from(CtEvent::Key(CtKeyEvent::new_with_kind(code, mods, kind)))
}

fn key(s: &str) -> Event {
    Event::Key(s.parse().unwrap())
}

#[test]
fn crossterm_modifiers() {
    const C: CtMods = CtMods::CONTROL;
    const S: CtMods = CtMods::SHIFT;
    const A: CtMods = CtMods::ALT;

    for (code, mods, expected) in [
        (CtCode::Char('x'), CtMods::NONE, "x"),
        (CtCode::Char('A'), C | S, "<C-S-a>"),
        (CtCode::Char('x'), C | A, "<C-A-x>"),
        (CtCode::Char('A'), C | S | A, "<C-S-A-a>"),
        (CtCode::BackTab, C | S, "<C-S-tab>"),
    ] {
        assert_eq!(convert(code, mods, KeyEventKind::Press), Ok(key(expected)), "{mods:?}");
    }

    for mods in [CtMods::SUPER, CtMods::HYPER, CtMods::META, C | CtMods::SUPER] {
        assert_eq!(convert(CtCode::Char('x'), mods, KeyEventKind::Press), Err(()), "{mods:?}");
    }
}

#[test]
fn crossterm_key_releases_are_dropped() {
    for (kind, expected) in [
        (KeyEventKind::Press, Ok(key("j"))),
        (KeyEventKind::Repeat, Ok(key("j"))),
        (KeyEventKind::Release, Err(())),
    ] {
        assert_eq!(convert(CtCode::Char('j'), CtMods::NONE, kind), expected, "{kind:?}");
    }
}

#[test]
fn display() {
    for (s, expected) in [
        ("<S-a>", "A"),
        ("<S-1>", "<S-1>"),
        ("<S-;>", "<S-;>"),
        ("<C-S-1>", "<S-C-1>"),
        ("<space>", "<space>"),
        ("<C-space>", "<C-space>"),
        ("<lt>", "<lt>"),
    ] {
        assert_eq!(s.parse::<KeyEvent>().unwrap().to_string(), expected, "`{s}`");
    }
}

#[test]
fn display_parse_round_trip() {
    let specials = [
        KeyCode::Backspace,
        KeyCode::Enter,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::Tab,
        KeyCode::Delete,
        KeyCode::Insert,
        KeyCode::Esc,
    ];
    let printable_ascii = (' '..='~').map(KeyCode::Char);
    let all_modifiers =
        (0..=KeyModifiers::all().bits()).map(KeyModifiers::from_bits_truncate).collect::<Vec<_>>();

    for code in specials.into_iter().chain(printable_ascii) {
        for &modifiers in &all_modifiers {
            let key = KeyEvent::new(code, modifiers);
            let s = key.to_string();
            assert_eq!(s.parse::<KeyEvent>(), Ok(key), "`{s}`");
        }
    }
}

#[test]
fn parse_key_event() {
    for s in [
        "a", "b", "<C-a>", "<C-S-d>", "A", "<BS>", "<C-BS>", "<c-cr>", "<c-CR>", "<C-a>",
        "<C-S-a>", "<C-S-A>", "<C-S-BS>", "<C-S-CR>", "Y",
    ] {
        let parsed = s.parse::<KeyEvent>().unwrap_or_else(|err| {
            panic!("case: `{s}` should parse but failed to parse with error: `{err:?}`")
        });
        let x = parsed.to_string();
        assert_eq!(parsed, x.parse::<KeyEvent>().unwrap(), "case: raw=`{s}` to_string=`{x}`");
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
        );
    }
}

#[test]
fn parse_key_sequence() {
    for s in [
        "Y",
        "S-y",
        "<C-r>s",
        "abcd<C-r>s",
        "<C-r><C-r>s",
        "<C-w>j",
        "<C-w>'",
        "<C-w>\"",
        ":",
        ":w",
        ":w<CR>",
        "<C-w>",
        "<C-w>j",
    ] {
        let parsed = s.parse::<KeySequence>().unwrap_or_else(|err| {
            panic!("case: `{s}` should parse but failed to parse with error: `{err:?}`")
        });
        let x = parsed.to_string();
        assert_eq!(parsed, x.parse::<KeySequence>().unwrap(), "case: raw=`{s}` to_string=`{x}`");
    }
}
