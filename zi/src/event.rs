mod events;
mod handler;

use std::any::{Any, TypeId};
use std::future::Future;
use std::sync::{Arc, OnceLock};

pub use self::events::*;
pub(crate) use self::handler::{async_handler, handler, AsyncEventHandler, EventHandler};
use self::handler::{ErasedAsyncEventHandler, ErasedEventHandler};
use crate::{Client, Editor, Result};

#[derive(Default)]
pub struct Registry {
    handlers: flurry::HashMap<TypeId, Vec<Arc<dyn ErasedEventHandler + Send + Sync>>>,
    async_handlers: flurry::HashMap<TypeId, Vec<Arc<dyn ErasedAsyncEventHandler + Send + Sync>>>,
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
    pub fn subscribe<T: Event>(&self, handler: impl EventHandler<Event = T>) {
        let handlers = self.handlers.pin();
        let id = TypeId::of::<T>();
        let handler = Arc::new(handler) as Arc<dyn ErasedEventHandler + Send + Sync>;
        if handlers
            .compute_if_present(&id, {
                let handler = handler.clone();
                |_k, v| {
                    let mut v = v.to_vec();
                    v.push(handler);
                    Some(v)
                }
            })
            .is_none()
        {
            handlers.insert(id, vec![handler]);
        }
    }

    pub async fn subscribe_async<T: AsyncEvent>(
        &self,
        handler: impl AsyncEventHandler<Event = T> + Sync,
    ) {
        let handlers = self.async_handlers.pin();
        let id = TypeId::of::<T>();
        let handler = Arc::new(handler) as Arc<dyn ErasedAsyncEventHandler + Send + Sync>;
        if handlers
            .compute_if_present(&id, {
                let handler = handler.clone();
                |_k, v| {
                    let mut v = v.to_vec();
                    v.push(handler);
                    Some(v)
                }
            })
            .is_none()
        {
            handlers.insert(id, vec![handler]);
        }
    }

    pub fn dispatch<T: Event>(&self, editor: &mut Editor, event: &T) {
        self.handlers.pin().compute_if_present(&TypeId::of::<T>(), |_, handlers| {
            let new = handlers
                .iter()
                .filter_map(|handler| match handler.dyn_on_event(editor, event) {
                    HandlerResult::Continue => Some(Arc::clone(&handler)),
                    HandlerResult::Unsubscribe => None,
                })
                .collect::<Vec<_>>();
            if new.is_empty() { None } else { Some(new) }
        });
    }

    pub async fn dispatch_async<T: AsyncEvent>(&self, client: &Client, event: &T) -> Result<()> {
        let handlers = self.async_handlers.pin();
        let id = TypeId::of::<T>();
        let handlers = handlers.get(&id).map(|handlers| handlers.clone());
        if let Some(handlers) = handlers {
            for handler in handlers {
                match handler.dyn_on_event(client.clone(), &event).await {
                    Ok(HandlerResult::Continue) => {}
                    Ok(HandlerResult::Unsubscribe) => {
                        handlers.remove(&handler);
                    }
                    Err(e) => return Err(e),
                }
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
pub trait Event: Any + Send + Sync {}

/// Marker trait for an asynchronous event.
pub trait AsyncEvent: Any + Clone + Send + Sync {}
