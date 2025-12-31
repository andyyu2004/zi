use std::fmt;
use std::str::FromStr;

use chumsky::Parser;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum Event {
    Key(KeyEvent),
    Resize(u16, u16),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum KeyCode {
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    Delete,
    Insert,
    Esc,
    F(u8),
    Char(char),
}

impl KeyCode {
    pub fn is_special(&self) -> bool {
        !matches!(self, KeyCode::Char(_))
    }
}

#[cfg(feature = "crossterm")]
impl TryFrom<crossterm::event::KeyCode> for KeyCode {
    type Error = ();

    fn try_from(code: crossterm::event::KeyCode) -> Result<Self, Self::Error> {
        match code {
            crossterm::event::KeyCode::Backspace => Ok(KeyCode::Backspace),
            crossterm::event::KeyCode::Enter => Ok(KeyCode::Enter),
            crossterm::event::KeyCode::Left => Ok(KeyCode::Left),
            crossterm::event::KeyCode::Right => Ok(KeyCode::Right),
            crossterm::event::KeyCode::Up => Ok(KeyCode::Up),
            crossterm::event::KeyCode::Down => Ok(KeyCode::Down),
            crossterm::event::KeyCode::Home => Ok(KeyCode::Home),
            crossterm::event::KeyCode::End => Ok(KeyCode::End),
            crossterm::event::KeyCode::PageUp => Ok(KeyCode::PageUp),
            crossterm::event::KeyCode::PageDown => Ok(KeyCode::PageDown),
            crossterm::event::KeyCode::Tab => Ok(KeyCode::Tab),
            crossterm::event::KeyCode::Delete => Ok(KeyCode::Delete),
            crossterm::event::KeyCode::Insert => Ok(KeyCode::Insert),
            crossterm::event::KeyCode::Esc => Ok(KeyCode::Esc),
            crossterm::event::KeyCode::F(n) => Ok(KeyCode::F(n)),
            crossterm::event::KeyCode::Char(c) => Ok(KeyCode::Char(c)),
            _ => Err(()),
        }
    }
}

impl fmt::Display for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyCode::Backspace => write!(f, "{}", keys::BACKSPACE),
            KeyCode::Enter => write!(f, "{}", keys::ENTER),
            KeyCode::Left => write!(f, "{}", keys::LEFT),
            KeyCode::Right => write!(f, "{}", keys::RIGHT),
            KeyCode::Up => write!(f, "{}", keys::UP),
            KeyCode::Down => write!(f, "{}", keys::DOWN),
            KeyCode::Home => write!(f, "{}", keys::HOME),
            KeyCode::End => write!(f, "{}", keys::END),
            KeyCode::PageUp => write!(f, "{}", keys::PAGEUP),
            KeyCode::PageDown => write!(f, "{}", keys::PAGEDOWN),
            KeyCode::Tab => write!(f, "{}", keys::TAB),
            KeyCode::Delete => write!(f, "{}", keys::DELETE),
            KeyCode::Insert => write!(f, "{}", keys::INSERT),
            KeyCode::Esc => write!(f, "{}", keys::ESC),
            KeyCode::F(n) => write!(f, "f{n}"),
            KeyCode::Char(c) => write!(f, "{c}"),
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash, Default)]
    #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
    pub struct KeyModifiers: u8 {
        const NONE = 0b0000_0000;
        const SHIFT = 0b0000_0001;
        const CONTROL = 0b0000_0010;
        const ALT = 0b0000_0100;
    }
}

#[cfg(feature = "crossterm")]
impl TryFrom<crossterm::event::KeyModifiers> for KeyModifiers {
    type Error = ();

    fn try_from(modifiers: crossterm::event::KeyModifiers) -> Result<Self, Self::Error> {
        Ok(match modifiers {
            crossterm::event::KeyModifiers::NONE => KeyModifiers::NONE,
            crossterm::event::KeyModifiers::SHIFT => KeyModifiers::SHIFT,
            crossterm::event::KeyModifiers::CONTROL => KeyModifiers::CONTROL,
            crossterm::event::KeyModifiers::ALT => KeyModifiers::ALT,
            _ => return Err(()),
        })
    }
}

impl From<KeyEvent> for Event {
    #[inline]
    fn from(v: KeyEvent) -> Self {
        Self::Key(v)
    }
}

impl From<char> for Event {
    #[inline]
    fn from(v: char) -> Self {
        Self::Key(KeyEvent::new(KeyCode::Char(v), KeyModifiers::empty()))
    }
}

#[cfg(feature = "crossterm")]
impl TryFrom<crossterm::event::Event> for Event {
    type Error = ();

    fn try_from(event: crossterm::event::Event) -> Result<Self, Self::Error> {
        match event {
            crossterm::event::Event::Key(event) => match event.code {
                // weird crossterm case, we just convert this to `<S-Tab>`
                crossterm::event::KeyCode::BackTab => Ok(Event::Key(KeyEvent::new(
                    KeyCode::Tab,
                    KeyModifiers::try_from(event.modifiers)? | KeyModifiers::SHIFT,
                ))),
                _ => Ok(Event::Key(KeyEvent::new(
                    event.code.try_into()?,
                    event.modifiers.try_into()?,
                ))),
            },

            crossterm::event::Event::Resize(width, height) => Ok(Event::Resize(width, height)),
            _ => Err(()),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct KeyEvent {
    code: KeyCode,
    modifiers: KeyModifiers,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for KeyEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(|mut errs: Vec<_>| serde::de::Error::custom(errs.swap_remove(0)))
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for KeyEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.to_string())
    }
}

impl FromStr for KeyEvent {
    type Err = Vec<chumsky::error::Simple<char>>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        key_event().then_ignore(chumsky::primitive::end()).parse(s)
    }
}

impl fmt::Display for KeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code {
            // special case, print <S-y> as just Y
            KeyCode::Char(c) if self.modifiers == KeyModifiers::SHIFT => {
                assert!(c.is_uppercase());
                return write!(f, "{c}");
            }
            _ => (),
        };

        if self.modifiers.is_empty() {
            if self.code.is_special() {
                write!(f, "<{}>", self.code)
            } else {
                write!(f, "{}", self.code)
            }
        } else {
            write!(f, "<")?;
            for modifier in self.modifiers.iter() {
                write!(
                    f,
                    "{}-",
                    match modifier {
                        KeyModifiers::CONTROL => "C",
                        KeyModifiers::SHIFT => "S",
                        KeyModifiers::ALT => "A",
                        _ => unreachable!("missing modifier case in fmt::Display for KeyEvent"),
                    }
                )?;
            }

            write!(f, "{}>", self.code)
        }
    }
}

impl KeyEvent {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        // normalize
        match code {
            // ensure capital letters have the shift modifier
            KeyCode::Char(c) if c.is_uppercase() => {
                KeyEvent { code: KeyCode::Char(c), modifiers: modifiers | KeyModifiers::SHIFT }
            }
            // <C-S-x> should be the same as <C-S-X>
            KeyCode::Char(c)
                if c.is_ascii_lowercase() && modifiers.contains(KeyModifiers::SHIFT) =>
            {
                KeyEvent { code: KeyCode::Char(c.to_ascii_uppercase()), modifiers }
            }
            _ => KeyEvent { code, modifiers },
        }
    }

    #[inline]
    pub fn code(&self) -> KeyCode {
        self.code
    }

    #[inline]
    pub fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }
}

impl From<KeyCode> for KeyEvent {
    #[inline]
    fn from(code: KeyCode) -> Self {
        Self { code, modifiers: KeyModifiers::empty() }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Default)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct KeySequence(Box<[KeyEvent]>);

impl FromIterator<KeyEvent> for KeySequence {
    fn from_iter<T: IntoIterator<Item = KeyEvent>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for KeySequence {
    type Item = KeyEvent;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_vec().into_iter()
    }
}

impl TryFrom<&str> for KeySequence {
    type Error = Vec<chumsky::error::Simple<char>>;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl fmt::Display for KeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for key in self.0.iter() {
            write!(f, "{key}")?;
        }
        Ok(())
    }
}

impl FromStr for KeySequence {
    type Err = Vec<chumsky::error::Simple<char>>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        key_event()
            .repeated()
            .at_least(1)
            .then_ignore(chumsky::primitive::end())
            .parse(s)
            .map(|x| KeySequence(x.into_boxed_slice()))
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for KeySequence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(|mut errs: Vec<_>| serde::de::Error::custom(errs.swap_remove(0)))
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for KeySequence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.to_string())
    }
}

fn key_event() -> impl Parser<char, KeyEvent, Error = chumsky::error::Simple<char>> {
    use chumsky::prelude::*;
    use chumsky::text::ident;

    // case insensitive variant of `keyword`
    fn kw(
        keyword: &'static str,
    ) -> impl Parser<char, (), Error = chumsky::error::Simple<char>> + Copy {
        ident()
            .try_map(move |s: String, span| {
                if s.eq_ignore_ascii_case(keyword) {
                    Ok(())
                } else {
                    Err(chumsky::error::Simple::expected_input_found(
                        span,
                        keyword.to_string().chars().map(Some),
                        s[..].chars().next(),
                    ))
                }
            })
            .ignored()
    }

    let special_key = choice((
        kw(keys::BACKSPACE).to(KeyCode::Backspace),
        kw(keys::ENTER).to(KeyCode::Enter),
        kw(keys::LEFT).to(KeyCode::Left),
        kw(keys::RIGHT).to(KeyCode::Right),
        kw(keys::UP).to(KeyCode::Up),
        kw(keys::DOWN).to(KeyCode::Down),
        kw(keys::HOME).to(KeyCode::Home),
        kw(keys::END).to(KeyCode::End),
        kw(keys::PAGEUP).to(KeyCode::PageUp),
        kw(keys::PAGEDOWN).to(KeyCode::PageDown),
        kw(keys::TAB).to(KeyCode::Tab),
        kw(keys::DELETE).to(KeyCode::Delete),
        kw(keys::INSERT).to(KeyCode::Insert),
        kw(keys::ESC).to(KeyCode::Esc),
        kw(keys::SPACE).to(KeyCode::Char(' ')),
        kw(keys::MINUS).to(KeyCode::Char('-')),
        kw(keys::LESS_THAN).to(KeyCode::Char('<')),
        kw(keys::GREATER_THAN).to(KeyCode::Char('>')),
    ));

    let key =
        filter(|c: &char| c.is_ascii_alphanumeric() || c.is_ascii_punctuation()).map(KeyCode::Char);

    let modifier = choice((
        kw("c").to(KeyModifiers::CONTROL),
        kw("s").to(KeyModifiers::SHIFT),
        kw("a").to(KeyModifiers::ALT),
    ));

    let modifiers = modifier
        .then_ignore(just("-"))
        .repeated()
        .at_least(1)
        .map(|x| x.into_iter().fold(KeyModifiers::empty(), |a, b| a | b));

    let modified_key = modifiers
        .then(choice((special_key, key)))
        .map(|(modifiers, code)| KeyEvent::new(code, modifiers))
        .delimited_by(just("<"), just(">"));

    let unmodified_key = choice((special_key.delimited_by(just("<"), just(">")), key))
        .map(|code| KeyEvent::new(code, KeyModifiers::empty()));

    choice((modified_key, unmodified_key))
}

pub(crate) mod keys {
    pub(crate) const BACKSPACE: &str = "bs";
    pub(crate) const ENTER: &str = "cr";
    pub(crate) const LEFT: &str = "left";
    pub(crate) const RIGHT: &str = "right";
    pub(crate) const UP: &str = "up";
    pub(crate) const DOWN: &str = "down";
    pub(crate) const HOME: &str = "home";
    pub(crate) const END: &str = "end";
    pub(crate) const PAGEUP: &str = "pageup";
    pub(crate) const PAGEDOWN: &str = "pagedown";
    pub(crate) const TAB: &str = "tab";
    pub(crate) const DELETE: &str = "del";
    pub(crate) const INSERT: &str = "ins";
    pub(crate) const ESC: &str = "esc";
    pub(crate) const SPACE: &str = "space";
    pub(crate) const MINUS: &str = "minus";
    pub(crate) const LESS_THAN: &str = "lt";
    pub(crate) const GREATER_THAN: &str = "gt";
}
