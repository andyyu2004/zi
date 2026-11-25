mod events;
mod handler;

use std::any::{Any, TypeId};
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use crossbeam_queue::SegQueue;
use rustc_hash::FxHashMap;
pub use zi_event::{Event, EventHandler, HandlerResult};

pub use self::events::*;
pub(crate) use self::handler::{AsyncEventHandler, async_handler};
use self::handler::{ErasedAsyncEventHandler, ErasedEventHandler};
use crate::{Client, Editor, Result};

type SubscriptionId = u64;

#[derive(Default)]
pub struct Registry {
    // map<event_type, [(subscription_id, handler)]> in subscription order
    #[allow(clippy::type_complexity)]
    handlers: parking_lot::RwLock<
        FxHashMap<TypeId, Vec<(SubscriptionId, Arc<dyn ErasedEventHandler + Send + Sync>)>>,
    >,
    // The list of handlers to remove.
    // The current strategy is to check this list when we subscribe a new handler.
    // There is no guarantee that we will ever clear this queue.
    garbage: SegQueue<(TypeId, SubscriptionId)>,
    async_garbage: SegQueue<(TypeId, SubscriptionId)>,

    #[allow(clippy::type_complexity)]
    async_handlers: tokio::sync::RwLock<
        FxHashMap<TypeId, Vec<(SubscriptionId, Arc<dyn ErasedAsyncEventHandler + Send + Sync>)>>,
    >,
    next_id: AtomicU64,
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

pub fn subscribe<E: Event>(handler: impl EventHandler<Editor, Event = E>) {
    registry().subscribe(handler)
}

pub fn subscribe_with<E: Event>(
    f: impl Fn(&mut Editor, &E) -> HandlerResult + Send + Sync + 'static,
) {
    subscribe(zi_event::handler(f));
}

pub async fn subscribe_async<E: AsyncEvent>(handler: impl AsyncEventHandler<Event = E>) {
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
        H: EventHandler<Editor, Event = T>,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut handlers = self.handlers.write();
        while let Some((event_type, id)) = self.garbage.pop() {
            handlers.get_mut(&event_type).unwrap().retain(|&(hid, _)| hid != id);
        }

        handlers.entry(TypeId::of::<T>()).or_default().push((id, Arc::new(handler)));
    }

    pub async fn subscribe_async<T, H>(&self, handler: H)
    where
        T: AsyncEvent,
        H: AsyncEventHandler<Event = T> + Sync,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut handlers = self.async_handlers.write().await;

        while let Some((event_type, id)) = self.async_garbage.pop() {
            handlers.get_mut(&event_type).unwrap().retain(|&(hid, _)| hid != id);
        }

        handlers.entry(TypeId::of::<T>()).or_default().push((id, Arc::new(handler)));
    }

    pub fn dispatch<T: Event>(&self, editor: &mut Editor, event: &T) {
        let handlers = self.handlers.read().get(&TypeId::of::<T>()).cloned();
        if let Some(handlers) = handlers {
            for (id, handler) in handlers {
                if handler.dyn_on_event(editor, event) == HandlerResult::Unsubscribe {
                    self.garbage.push((TypeId::of::<T>(), id));
                }
            }
        }
    }

    pub async fn dispatch_async<T: AsyncEvent>(&self, client: &Client, event: &T) -> Result<()> {
        let handlers = self.async_handlers.read().await.get(&TypeId::of::<T>()).cloned();
        if let Some(handlers) = handlers {
            for (id, handler) in handlers {
                if handler.dyn_on_event(client, event).await? == HandlerResult::Unsubscribe {
                    self.async_garbage.push((TypeId::of::<T>(), id));
                }
            }
        }

        Ok(())
    }
}

pub type AsyncHandlerResult = Result<HandlerResult>;

/// Marker trait for an asynchronous event.
/// This means the handlers can be async functions and the
/// `dispatch_async` call must be awaited.
pub trait AsyncEvent: Any + Clone + Send + Sync {}
