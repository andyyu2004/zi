use async_trait::async_trait;

use super::*;
use crate::Client;

#[async_trait]
pub trait AsyncEventHandler: Send + 'static {
    type Event: AsyncEvent;

    async fn on_event(&mut self, client: Client, event: Self::Event) -> HandlerResult;
}

#[async_trait]
pub(super) trait ErasedAsyncEventHandler: Send + 'static {
    async fn dyn_on_event(
        &mut self,
        client: Client,
        event: &(dyn Any + Send + Sync),
    ) -> HandlerResult;
}

#[async_trait]
impl<H> ErasedAsyncEventHandler for H
where
    H: AsyncEventHandler,
{
    async fn dyn_on_event(
        &mut self,
        client: Client,
        event: &(dyn Any + Send + Sync),
    ) -> HandlerResult {
        if let Some(event) = (event as &dyn Any).downcast_ref::<H::Event>() {
            return self.on_event(client.clone(), event.clone()).await;
        }

        HandlerResult::Ok
    }
}

struct AsyncHandlerFunc<F, Fut, E> {
    f: F,
    _marker: std::marker::PhantomData<(Fut, E)>,
}

#[async_trait]
impl<F, E, Fut> AsyncEventHandler for AsyncHandlerFunc<F, Fut, E>
where
    F: FnMut(Client, E) -> Fut + Send + 'static,
    E: AsyncEvent,
    Fut: Future<Output = HandlerResult> + Send + 'static,
{
    type Event = E;

    async fn on_event(&mut self, client: Client, event: E) -> HandlerResult {
        (self.f)(client, event).await
    }
}

/// Create a new event handler from a closure.
// Can't find a way to implement this as a blanket impl
pub(crate) fn async_handler<E, Fut>(
    f: impl FnMut(Client, E) -> Fut + Send + 'static,
) -> impl AsyncEventHandler<Event = E>
where
    E: AsyncEvent,
    Fut: Future<Output = HandlerResult> + Send + 'static,
{
    AsyncHandlerFunc::<_, Fut, E> { f, _marker: std::marker::PhantomData }
}

pub trait EventHandler: Send + 'static {
    type Event: Event;

    fn on_event(&mut self, editor: &mut Editor, event: &Self::Event) -> HandlerResult;
}

pub(super) trait ErasedEventHandler: Send + 'static {
    fn dyn_on_event(&mut self, editor: &mut Editor, event: &dyn Event) -> HandlerResult;
}

impl<H> ErasedEventHandler for H
where
    H: EventHandler,
{
    fn dyn_on_event(&mut self, editor: &mut Editor, event: &dyn Event) -> HandlerResult {
        if let Some(event) = (event as &dyn Any).downcast_ref::<H::Event>() {
            return self.on_event(editor, event);
        }

        HandlerResult::Ok
    }
}

struct HandlerFunc<F, E> {
    f: F,
    _marker: std::marker::PhantomData<E>,
}

impl<F, E> EventHandler for HandlerFunc<F, E>
where
    F: FnMut(&mut Editor, &E) -> HandlerResult + Send + 'static,
    E: Event,
{
    type Event = E;

    fn on_event(&mut self, editor: &mut Editor, event: &E) -> HandlerResult {
        (self.f)(editor, event)
    }
}

/// Create a new event handler from a closure.
// Can't find a way to implement this as a blanket impl
pub(crate) fn handler<E: Event>(
    f: impl FnMut(&mut Editor, &E) -> HandlerResult + Send + 'static,
) -> impl EventHandler<Event = E> {
    HandlerFunc { f, _marker: std::marker::PhantomData }
}
