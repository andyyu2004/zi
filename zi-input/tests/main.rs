use crossterm::event as ct;
use zi_input::{Event, KeyCode, KeyEvent, KeyModifiers, KeySequence};

fn all_modifiers() -> impl Iterator<Item = KeyModifiers> {
    (0..=KeyModifiers::all().bits()).map(KeyModifiers::from_bits_truncate)
}

#[test]
fn crossterm_modifier_combinations() {
    for (ct_mods, expected) in [
        (ct::KeyModifiers::NONE, KeyModifiers::NONE),
        (
            ct::KeyModifiers::CONTROL | ct::KeyModifiers::SHIFT,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ),
        (
            ct::KeyModifiers::CONTROL | ct::KeyModifiers::ALT,
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ),
        (ct::KeyModifiers::SHIFT | ct::KeyModifiers::ALT, KeyModifiers::SHIFT | KeyModifiers::ALT),
        (
            ct::KeyModifiers::CONTROL | ct::KeyModifiers::SHIFT | ct::KeyModifiers::ALT,
            KeyModifiers::all(),
        ),
    ] {
        assert_eq!(KeyModifiers::try_from(ct_mods), Ok(expected), "case: {ct_mods:?}");
    }

    for unsupported in [
        ct::KeyModifiers::SUPER,
        ct::KeyModifiers::HYPER,
        ct::KeyModifiers::META,
        ct::KeyModifiers::CONTROL | ct::KeyModifiers::SUPER,
    ] {
        assert_eq!(KeyModifiers::try_from(unsupported), Err(()), "case: {unsupported:?}");
    }
}

#[test]
fn crossterm_ctrl_shift_key_event() {
    let event = ct::Event::Key(ct::KeyEvent::new(
        ct::KeyCode::Char('A'),
        ct::KeyModifiers::CONTROL | ct::KeyModifiers::SHIFT,
    ));
    assert_eq!(Event::try_from(event), Ok(Event::Key("<C-S-a>".parse().unwrap())));

    let event = ct::Event::Key(ct::KeyEvent::new(
        ct::KeyCode::Char('x'),
        ct::KeyModifiers::CONTROL | ct::KeyModifiers::ALT,
    ));
    assert_eq!(Event::try_from(event), Ok(Event::Key("<C-A-x>".parse().unwrap())));

    let event = ct::Event::Key(ct::KeyEvent::new(
        ct::KeyCode::BackTab,
        ct::KeyModifiers::CONTROL | ct::KeyModifiers::SHIFT,
    ));
    assert_eq!(Event::try_from(event), Ok(Event::Key("<C-S-tab>".parse().unwrap())));
}

#[test]
fn crossterm_key_event_kinds() {
    let key = |kind| {
        Event::try_from(ct::Event::Key(ct::KeyEvent::new_with_kind(
            ct::KeyCode::Char('j'),
            ct::KeyModifiers::NONE,
            kind,
        )))
    };
    assert_eq!(key(ct::KeyEventKind::Press), Ok(Event::from('j')));
    assert_eq!(key(ct::KeyEventKind::Repeat), Ok(Event::from('j')));
    assert_eq!(key(ct::KeyEventKind::Release), Err(()));
}

#[test]
fn display_shift_non_letter() {
    for (s, expected) in
        [("<S-1>", "<S-1>"), ("<S-;>", "<S-;>"), ("<S-a>", "A"), ("<C-S-1>", "<S-C-1>")]
    {
        let key = s.parse::<KeyEvent>().unwrap();
        assert_eq!(key.to_string(), expected, "case: `{s}`");
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
    let chars = (' '..='~').map(KeyCode::Char);

    for code in specials.into_iter().chain(chars) {
        for modifiers in all_modifiers() {
            let key = KeyEvent::new(code, modifiers);
            let s = key.to_string();
            let parsed = s.parse::<KeyEvent>().unwrap_or_else(|err| {
                panic!("`{s}` from {key:?} should parse but failed with `{err:?}`")
            });
            assert_eq!(parsed, key, "case: `{s}`");
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
