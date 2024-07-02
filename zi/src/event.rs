mod events;
mod handler;

use std::any::{Any, TypeId};
use std::future::Future;
use std::sync::{Arc, OnceLock};

use crossbeam_queue::SegQueue;
use rustc_hash::FxHashMap;

pub use self::events::*;
pub(crate) use self::handler::{async_handler, handler, AsyncEventHandler, EventHandler};
use self::handler::{ErasedAsyncEventHandler, ErasedEventHandler};
use crate::{Client, Editor, Result};

#[derive(Default)]
pub struct Registry {
    // map<event_type, map<handler_type, handler>>
    // We key by handler type too to avoid duplicates in tests as this is stored in a static.
    handlers: parking_lot::RwLock<
        FxHashMap<TypeId, FxHashMap<TypeId, Arc<dyn ErasedEventHandler + Send + Sync>>>,
    >,
    // The list of handlers to remove.
    // The current strategy is to check this list when we subscribe a new handler.
    // There is no guarantee that we will ever clear this queue.
    // This could be a problem in theory if we keep attempting to subscribe the same handler.
    garbage: SegQueue<(TypeId, TypeId)>,
    async_garbage: SegQueue<(TypeId, TypeId)>,

    async_handlers: tokio::sync::RwLock<
        FxHashMap<TypeId, FxHashMap<TypeId, Arc<dyn ErasedAsyncEventHandler + Send + Sync>>>,
    >,
}

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(Registry::default)
}

pub fn dispatch(editor: &mut Editor, event: impl Event) {
    registry().dispatch(editor, &event)
}

pub async fn dispatch_async(client: &Client, event: impl AsyncEvent) -> Result<()> {
    registry().dispatch_async(client, &event).await
}

pub fn subscribe<T: Event>(handler: impl EventHandler<Event = T>) {
    registry().subscribe(handler)
}

pub fn subscribe_with<E: Event>(
    f: impl Fn(&mut Editor, &E) -> HandlerResult + Send + Sync + 'static,
) {
    subscribe(handler(f));
}

pub async fn subscribe_async<E: AsyncEvent>(handler: impl AsyncEventHandler<Event = E> + Sync) {
    registry().subscribe_async(handler).await
}

pub async fn subscribe_async_with<E, Fut>(f: impl Fn(Client, E) -> Fut + Send + Sync + 'static)
where
    E: AsyncEvent,
    Fut: Future<Output = AsyncHandlerResult> + Send + 'static,
{
    subscribe_async(async_handler(f)).await
}

impl Registry {
    pub fn subscribe<T, H>(&self, handler: H)
    where
        T: Event,
        H: EventHandler<Event = T>,
    {
        let mut handlers = self.handlers.write();
        while let Some((event_type, handler_type)) = self.garbage.pop() {
            handlers.get_mut(&event_type).unwrap().remove(&handler_type);
        }

        handlers.entry(TypeId::of::<T>()).or_default().insert(TypeId::of::<H>(), Arc::new(handler));
    }

    pub async fn subscribe_async<T, H>(&self, handler: H)
    where
        T: AsyncEvent,
        H: AsyncEventHandler<Event = T> + Sync,
    {
        let mut handlers = self.async_handlers.write().await;

        while let Some((event_type, handler_type)) = self.async_garbage.pop() {
            handlers.get_mut(&event_type).unwrap().remove(&handler_type);
        }

        handlers.entry(TypeId::of::<T>()).or_default().insert(TypeId::of::<H>(), Arc::new(handler));
    }

    pub fn dispatch<T: Event>(&self, editor: &mut Editor, event: &T) {
        if let Some(handlers) = self.handlers.read().get(&TypeId::of::<T>()) {
            for (&hty, handler) in handlers {
                if handler.dyn_on_event(editor, event) == HandlerResult::Unsubscribe {
                    self.garbage.push((TypeId::of::<T>(), hty));
                }
            }
        }
    }

    pub async fn dispatch_async<T: AsyncEvent>(&self, client: &Client, event: &T) -> Result<()> {
        if let Some(handlers) = self.async_handlers.read().await.get(&TypeId::of::<T>()) {
            for (&hty, handler) in handlers {
                if handler.dyn_on_event(client, event).await? == HandlerResult::Unsubscribe {
                    self.async_garbage.push((TypeId::of::<T>(), hty));
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerResult {
    Continue,
    /// Queue the handler for removal.
    Unsubscribe,
}

pub type AsyncHandlerResult = Result<HandlerResult>;

/// Marker trait for a synchronous event.
pub trait Event: Any + Send + Sync {}

/// Marker trait for an asynchronous event.
pub trait AsyncEvent: Any + Clone + Send + Sync {}
