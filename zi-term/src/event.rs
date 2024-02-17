/// Minimized subset of `crossterm::event::Event` as we don't care about all the fields

pub enum Event {
    Key(zi::event::KeyEvent),
    Resize(u16, u16),
}

impl Event {
    pub fn from_crossterm(event: crossterm::event::Event) -> Option<Self> {
        match event {
            crossterm::event::Event::Key(event) => Some(Event::Key(zi::event::KeyEvent {
                code: event.code,
                modifiers: event.modifiers,
            })),
            crossterm::event::Event::Resize(width, height) => Some(Event::Resize(width, height)),
            _ => None,
        }
    }
}
