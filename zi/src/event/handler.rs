use std::marker::PhantomData;

use async_trait::async_trait;
use zi_event::EventHandler;

use super::*;
use crate::Client;

#[async_trait]
pub trait AsyncEventHandler: Send + Sync + 'static {
    type Event: AsyncEvent;

    async fn on_event(&self, client: Client, event: Self::Event) -> AsyncHandlerResult;
}

#[async_trait]
pub(super) trait ErasedAsyncEventHandler: Send + 'static {
    async fn dyn_on_event(
        &self,
        client: &Client,
        event: &(dyn Any + Send + Sync),
    ) -> AsyncHandlerResult;
}

#[async_trait]
impl<H> ErasedAsyncEventHandler for H
where
    H: AsyncEventHandler,
{
    async fn dyn_on_event(
        &self,
        client: &Client,
        event: &(dyn Any + Send + Sync),
    ) -> AsyncHandlerResult {
        if let Some(event) = (event as &dyn Any).downcast_ref::<H::Event>() {
            return self.on_event(client.clone(), event.clone()).await;
        }

        Ok(HandlerResult::Continue)
    }
}

struct AsyncHandlerFunc<F, Fut, E> {
    f: F,
    #[allow(clippy::type_complexity)]
    _marker: PhantomData<(fn() -> Fut, fn() -> E)>,
}

/// `Fut` shouldn't need to be sync for the handler to be sync
unsafe impl<F, Fut, E> Sync for AsyncHandlerFunc<F, Fut, E>
where
    F: Sync,
    E: Sync,
{
}

#[async_trait]
impl<F, E, Fut> AsyncEventHandler for AsyncHandlerFunc<F, Fut, E>
where
    F: Fn(Client, E) -> Fut + Send + Sync + 'static,
    E: AsyncEvent,
    Fut: Future<Output = AsyncHandlerResult> + Send + 'static,
{
    type Event = E;

    async fn on_event(&self, client: Client, event: E) -> AsyncHandlerResult {
        (self.f)(client, event).await
    }
}

/// Create a new event handler from a closure.
// Can't find a way to implement this as a blanket impl
pub(crate) fn async_handler<E, Fut>(
    f: impl Fn(Client, E) -> Fut + Send + Sync + 'static,
) -> impl AsyncEventHandler<Event = E>
where
    E: AsyncEvent,
    Fut: Future<Output = AsyncHandlerResult> + Send + 'static,
{
    AsyncHandlerFunc::<_, Fut, E> { f, _marker: PhantomData }
}

// pub trait EventHandler: Send + Sync + 'static {
//     type Event: Event;
//
//     fn on_event(&self, editor: &mut Editor, event: &Self::Event) -> HandlerResult;
// }

pub(super) trait ErasedEventHandler: Send + 'static {
    fn dyn_on_event(&self, editor: &mut Editor, event: &dyn Event) -> HandlerResult;
}

impl<H> ErasedEventHandler for H
where
    H: EventHandler<Editor>,
{
    fn dyn_on_event(&self, editor: &mut Editor, event: &dyn Event) -> HandlerResult {
        if let Some(event) = (event as &dyn Any).downcast_ref::<H::Event>() {
            return self.on_event(editor, event);
        }

        HandlerResult::Continue
    }
}
