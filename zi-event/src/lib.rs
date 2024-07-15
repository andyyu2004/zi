use std::any::Any;

pub trait Registry<X> {
    fn subscribe<E: Event>(&self, handler: impl EventHandler<X, Event = E>);
}

pub trait Event: Send + Sync + Any {}

pub trait EventHandler<X>: Send + Sync + 'static {
    type Event: Event;

    fn on_event(&self, cx: &mut X, event: &Self::Event) -> HandlerResult;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerResult {
    Continue,
    /// Queue the handler for removal.
    Unsubscribe,
}
