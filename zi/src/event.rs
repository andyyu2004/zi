use std::any::{Any, TypeId};
use std::sync::OnceLock;

use parking_lot::Mutex;
use rustc_hash::FxHashMap;

use crate::{BufferId, Editor};

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
