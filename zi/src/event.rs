mod events;
mod handler;

use std::any::{Any, TypeId};
use std::future::Future;
use std::sync::OnceLock;

use rustc_hash::FxHashMap;

pub use self::events::*;
use self::handler::{
    async_handler, handler, AsyncEventHandler, ErasedAsyncEventHandler, ErasedEventHandler,
    EventHandler,
};
use crate::{Client, Editor, Result};

#[derive(Default)]
pub struct Registry {
    handlers: parking_lot::Mutex<FxHashMap<TypeId, Vec<Box<dyn ErasedEventHandler + Send>>>>,
    async_handlers:
        tokio::sync::Mutex<FxHashMap<TypeId, Vec<Box<dyn ErasedAsyncEventHandler + Send>>>>,
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

pub fn subscribe_with<E: Event>(f: impl FnMut(&mut Editor, &E) -> HandlerResult + Send + 'static) {
    subscribe(handler(f));
}

pub async fn subscribe_async<E: AsyncEvent>(handler: impl AsyncEventHandler<Event = E>) {
    registry().subscribe_async(handler).await
}

pub async fn subscribe_async_with<E, Fut>(f: impl FnMut(Client, E) -> Fut + Send + 'static)
where
    E: AsyncEvent,
    Fut: Future<Output = AsyncHandlerResult> + Send + 'static,
{
    subscribe_async(async_handler(f)).await
}

impl Registry {
    pub fn subscribe<T: Event>(&self, handler: impl EventHandler<Event = T>) {
        self.handlers.lock().entry(TypeId::of::<T>()).or_default().push(Box::new(handler));
    }

    pub async fn subscribe_async<T: AsyncEvent>(&self, handler: impl AsyncEventHandler<Event = T>) {
        self.async_handlers
            .lock()
            .await
            .entry(TypeId::of::<T>())
            .or_default()
            .push(Box::new(handler));
    }

    pub fn dispatch<T: Event>(&self, editor: &mut Editor, event: &T) {
        if let Some(handlers) = self.handlers.lock().get_mut(&TypeId::of::<T>()) {
            handlers.retain_mut(|handler| match handler.dyn_on_event(editor, event) {
                HandlerResult::Continue => true,
                HandlerResult::Unsubscribe => false,
            });
        }
    }

    pub async fn dispatch_async<T: AsyncEvent>(&self, client: &Client, event: &T) -> Result<()> {
        if let Some(handlers) = self.async_handlers.lock().await.get_mut(&TypeId::of::<T>()) {
            let mut indices_to_remove = vec![];
            for (i, handler) in handlers.iter_mut().enumerate() {
                match handler.dyn_on_event(client.clone(), event).await {
                    Ok(HandlerResult::Continue) => (),
                    Ok(HandlerResult::Unsubscribe) => indices_to_remove.push(i),
                    Err(err) => {
                        for i in indices_to_remove.into_iter().rev() {
                            handlers.remove(i);
                        }
                        return Err(err);
                    }
                }
            }

            for i in indices_to_remove.into_iter().rev() {
                handlers.remove(i);
            }
        }

        Ok(())
    }
}

#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerResult {
    /// Continue processing the event.
    Continue,
    /// Unsubscribe the handler from the event.
    Unsubscribe,
}

pub type AsyncHandlerResult = Result<HandlerResult>;

/// Marker trait for a synchronous event.
pub trait Event: Any + Send {}

/// Marker trait for an asynchronous event.
pub trait AsyncEvent: Any + Clone + Send + Sync {}
