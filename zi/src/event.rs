use std::any::{Any, TypeId};
use std::str::FromStr;
use std::sync::OnceLock;

pub use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::event::{MediaKeyCode, ModifierKeyCode};
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

use crate::{BufferId, Editor};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl FromStr for KeyEvent {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens: Vec<_> = s.split('-').collect();
        let mut code = match tokens.pop().ok_or_else(|| anyhow::anyhow!("Missing key code"))? {
            keys::BACKSPACE => KeyCode::Backspace,
            keys::ENTER => KeyCode::Enter,
            keys::LEFT => KeyCode::Left,
            keys::RIGHT => KeyCode::Right,
            keys::UP => KeyCode::Up,
            keys::DOWN => KeyCode::Down,
            keys::HOME => KeyCode::Home,
            keys::END => KeyCode::End,
            keys::PAGEUP => KeyCode::PageUp,
            keys::PAGEDOWN => KeyCode::PageDown,
            keys::TAB => KeyCode::Tab,
            keys::DELETE => KeyCode::Delete,
            keys::INSERT => KeyCode::Insert,
            keys::NULL => KeyCode::Null,
            keys::ESC => KeyCode::Esc,
            keys::SPACE => KeyCode::Char(' '),
            keys::MINUS => KeyCode::Char('-'),
            keys::LESS_THAN => KeyCode::Char('<'),
            keys::GREATER_THAN => KeyCode::Char('>'),
            keys::CAPS_LOCK => KeyCode::CapsLock,
            keys::SCROLL_LOCK => KeyCode::ScrollLock,
            keys::NUM_LOCK => KeyCode::NumLock,
            keys::PRINT_SCREEN => KeyCode::PrintScreen,
            keys::PAUSE => KeyCode::Pause,
            keys::MENU => KeyCode::Menu,
            keys::KEYPAD_BEGIN => KeyCode::KeypadBegin,
            keys::PLAY => KeyCode::Media(MediaKeyCode::Play),
            keys::PAUSE_MEDIA => KeyCode::Media(MediaKeyCode::Pause),
            keys::PLAY_PAUSE => KeyCode::Media(MediaKeyCode::PlayPause),
            keys::STOP => KeyCode::Media(MediaKeyCode::Stop),
            keys::REVERSE => KeyCode::Media(MediaKeyCode::Reverse),
            keys::FAST_FORWARD => KeyCode::Media(MediaKeyCode::FastForward),
            keys::REWIND => KeyCode::Media(MediaKeyCode::Rewind),
            keys::TRACK_NEXT => KeyCode::Media(MediaKeyCode::TrackNext),
            keys::TRACK_PREVIOUS => KeyCode::Media(MediaKeyCode::TrackPrevious),
            keys::RECORD => KeyCode::Media(MediaKeyCode::Record),
            keys::LOWER_VOLUME => KeyCode::Media(MediaKeyCode::LowerVolume),
            keys::RAISE_VOLUME => KeyCode::Media(MediaKeyCode::RaiseVolume),
            keys::MUTE_VOLUME => KeyCode::Media(MediaKeyCode::MuteVolume),
            keys::LEFT_SHIFT => KeyCode::Modifier(ModifierKeyCode::LeftShift),
            keys::LEFT_CONTROL => KeyCode::Modifier(ModifierKeyCode::LeftControl),
            keys::LEFT_ALT => KeyCode::Modifier(ModifierKeyCode::LeftAlt),
            keys::LEFT_SUPER => KeyCode::Modifier(ModifierKeyCode::LeftSuper),
            keys::LEFT_HYPER => KeyCode::Modifier(ModifierKeyCode::LeftHyper),
            keys::LEFT_META => KeyCode::Modifier(ModifierKeyCode::LeftMeta),
            keys::RIGHT_SHIFT => KeyCode::Modifier(ModifierKeyCode::RightShift),
            keys::RIGHT_CONTROL => KeyCode::Modifier(ModifierKeyCode::RightControl),
            keys::RIGHT_ALT => KeyCode::Modifier(ModifierKeyCode::RightAlt),
            keys::RIGHT_SUPER => KeyCode::Modifier(ModifierKeyCode::RightSuper),
            keys::RIGHT_HYPER => KeyCode::Modifier(ModifierKeyCode::RightHyper),
            keys::RIGHT_META => KeyCode::Modifier(ModifierKeyCode::RightMeta),
            keys::ISO_LEVEL_3_SHIFT => KeyCode::Modifier(ModifierKeyCode::IsoLevel3Shift),
            keys::ISO_LEVEL_5_SHIFT => KeyCode::Modifier(ModifierKeyCode::IsoLevel5Shift),
            single if single.chars().count() == 1 => KeyCode::Char(single.chars().next().unwrap()),
            function if function.len() > 1 && function.starts_with('F') => {
                let function: String = function.chars().skip(1).collect();
                let function = str::parse::<u8>(&function)?;
                (function > 0 && function < 25)
                    .then_some(KeyCode::F(function))
                    .ok_or_else(|| anyhow::anyhow!("Invalid function key '{function}'"))?
            }
            invalid => return Err(anyhow::anyhow!("Invalid key code '{invalid}'")),
        };

        let mut modifiers = KeyModifiers::empty();
        for token in tokens {
            let flag = match token {
                "S" => KeyModifiers::SHIFT,
                "A" => KeyModifiers::ALT,
                "C" => KeyModifiers::CONTROL,
                _ => return Err(anyhow::anyhow!("Invalid key modifier '{}-'", token)),
            };

            if modifiers.contains(flag) {
                return Err(anyhow::anyhow!("Repeated key modifier '{}-'", token));
            }
            modifiers.insert(flag);
        }

        // Normalize character keys so that characters like C-S-r and C-R
        // are represented by equal KeyEvents.
        match code {
            KeyCode::Char(ch)
                if ch.is_ascii_lowercase() && modifiers.contains(KeyModifiers::SHIFT) =>
            {
                code = KeyCode::Char(ch.to_ascii_uppercase());
                modifiers.remove(KeyModifiers::SHIFT);
            }
            _ => (),
        }

        Ok(KeyEvent { code, modifiers })
    }
}

pub struct Registry {
    handlers: FxHashMap<TypeId, Vec<Box<dyn ErasedEventHandler + Send + Sync>>>,
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn with(f: impl FnOnce(&mut Registry)) {
    f(&mut REGISTRY.get_or_init(|| Mutex::new(Registry::new())).lock());
}

pub fn dispatch(editor: &mut Editor, event: impl Event) {
    with(|registry| registry.dispatch(editor, &event));
}

pub fn register<T: Event>(handler: impl EventHandler<Event = T> + Send + Sync + 'static) {
    with(|registry| registry.register(handler));
}

/// Create a new event handler from a closure.
pub fn handler<E: Event>(f: impl FnMut(&mut Editor, &E)) -> impl EventHandler<Event = E> {
    HandlerFunc { f, _marker: std::marker::PhantomData }
}

impl Registry {
    fn new() -> Self {
        Self { handlers: FxHashMap::default() }
    }

    pub fn register<T: Event>(
        &mut self,
        handler: impl EventHandler<Event = T> + Send + Sync + 'static,
    ) {
        self.handlers.entry(TypeId::of::<T>()).or_default().push(Box::new(handler));
    }

    pub fn dispatch<T: Event>(&mut self, editor: &mut Editor, event: &T) {
        if let Some(handlers) = self.handlers.get_mut(&TypeId::of::<T>()) {
            for handler in handlers {
                handler.dyn_on_event(editor, event);
            }
        }
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

pub trait EventHandler {
    type Event: Event;

    fn on_event(&mut self, editor: &mut Editor, event: &Self::Event);
}

struct HandlerFunc<F, E> {
    f: F,
    _marker: std::marker::PhantomData<E>,
}

impl<F, E> EventHandler for HandlerFunc<F, E>
where
    F: FnMut(&mut Editor, &E),
    E: Event,
{
    type Event = E;

    fn on_event(&mut self, editor: &mut Editor, event: &E) {
        (self.f)(editor, event);
    }
}

trait ErasedEventHandler {
    fn dyn_on_event(&mut self, editor: &mut Editor, event: &dyn Event);
}

impl<H> ErasedEventHandler for H
where
    H: EventHandler,
{
    fn dyn_on_event(&mut self, editor: &mut Editor, event: &dyn Event) {
        if let Some(event) = (event as &dyn Any).downcast_ref::<H::Event>() {
            self.on_event(editor, event);
        }
    }
}

pub trait Event: Any + Send {}

#[derive(Debug)]
pub struct DidChangeBuffer {
    pub buf: BufferId,
}

impl Event for DidChangeBuffer {}

#[derive(Debug)]
pub struct DidOpenBuffer {
    pub buf: BufferId,
}

impl Event for DidOpenBuffer {}

pub(crate) mod keys {
    pub(crate) const BACKSPACE: &str = "backspace";
    pub(crate) const ENTER: &str = "ret";
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
    pub(crate) const NULL: &str = "null";
    pub(crate) const ESC: &str = "esc";
    pub(crate) const SPACE: &str = "space";
    pub(crate) const MINUS: &str = "minus";
    pub(crate) const LESS_THAN: &str = "lt";
    pub(crate) const GREATER_THAN: &str = "gt";
    pub(crate) const CAPS_LOCK: &str = "capslock";
    pub(crate) const SCROLL_LOCK: &str = "scrolllock";
    pub(crate) const NUM_LOCK: &str = "numlock";
    pub(crate) const PRINT_SCREEN: &str = "printscreen";
    pub(crate) const PAUSE: &str = "pause";
    pub(crate) const MENU: &str = "menu";
    pub(crate) const KEYPAD_BEGIN: &str = "keypadbegin";
    pub(crate) const PLAY: &str = "play";
    pub(crate) const PAUSE_MEDIA: &str = "pausemedia";
    pub(crate) const PLAY_PAUSE: &str = "playpause";
    pub(crate) const REVERSE: &str = "reverse";
    pub(crate) const STOP: &str = "stop";
    pub(crate) const FAST_FORWARD: &str = "fastforward";
    pub(crate) const REWIND: &str = "rewind";
    pub(crate) const TRACK_NEXT: &str = "tracknext";
    pub(crate) const TRACK_PREVIOUS: &str = "trackprevious";
    pub(crate) const RECORD: &str = "record";
    pub(crate) const LOWER_VOLUME: &str = "lowervolume";
    pub(crate) const RAISE_VOLUME: &str = "raisevolume";
    pub(crate) const MUTE_VOLUME: &str = "mutevolume";
    pub(crate) const LEFT_SHIFT: &str = "leftshift";
    pub(crate) const LEFT_CONTROL: &str = "leftcontrol";
    pub(crate) const LEFT_ALT: &str = "leftalt";
    pub(crate) const LEFT_SUPER: &str = "leftsuper";
    pub(crate) const LEFT_HYPER: &str = "lefthyper";
    pub(crate) const LEFT_META: &str = "leftmeta";
    pub(crate) const RIGHT_SHIFT: &str = "rightshift";
    pub(crate) const RIGHT_CONTROL: &str = "rightcontrol";
    pub(crate) const RIGHT_ALT: &str = "rightalt";
    pub(crate) const RIGHT_SUPER: &str = "rightsuper";
    pub(crate) const RIGHT_HYPER: &str = "righthyper";
    pub(crate) const RIGHT_META: &str = "rightmeta";
    pub(crate) const ISO_LEVEL_3_SHIFT: &str = "isolevel3shift";
    pub(crate) const ISO_LEVEL_5_SHIFT: &str = "isolevel5shift";
}
