pub mod event;

use std::any::Any;
use std::marker::PhantomData;

pub trait Registry<X: Send + 'static> {
    fn subscribe<E: Event>(&self, handler: impl EventHandler<X, Event = E>);

    fn subscribe_with<E: Event>(
        &self,
        f: impl Fn(&mut X, &E) -> HandlerResult + Send + Sync + 'static,
    ) {
        self.subscribe(handler(f));
    }
}

pub trait Event: Send + Sync + Any {}

pub trait EventHandler<X>: Send + Sync + 'static {
    type Event: Event;

    fn on_event(&self, cx: &mut X, event: &Self::Event) -> HandlerResult;
}

struct HandlerFunc<F, E, X> {
    f: F,
    _event: PhantomData<fn() -> E>,
    _editor: PhantomData<fn() -> X>,
}

impl<F, E, X> EventHandler<X> for HandlerFunc<F, E, X>
where
    F: Fn(&mut X, &E) -> HandlerResult + Send + Sync + 'static,
    E: Event,
    X: Send + 'static,
{
    type Event = E;

    fn on_event(&self, editor: &mut X, event: &E) -> HandlerResult {
        (self.f)(editor, event)
    }
}

/// Create a new event handler from a closure.
pub fn handler<X: Send + 'static, E: Event>(
    f: impl Fn(&mut X, &E) -> HandlerResult + Send + Sync + 'static,
) -> impl EventHandler<X, Event = E> {
    HandlerFunc { f, _event: PhantomData, _editor: PhantomData }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerResult {
    Continue,
    /// Queue the handler for removal.
    Unsubscribe,
}
