use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zi::event::{self, Event, HandlerResult};

use crate::new;

#[derive(Debug, Clone)]
struct SameHandlerTypeEvent;

impl Event for SameHandlerTypeEvent {}

#[derive(Debug, Clone)]
struct UnsubscribeOneEvent;

impl Event for UnsubscribeOneEvent {}

#[derive(Debug, Clone)]
struct UnrelatedEvent;

impl Event for UnrelatedEvent {}

fn subscribe_counter<E: Event>(count: Arc<AtomicUsize>, unsubscribe: bool) {
    event::subscribe_with::<E>(move |_editor, _event| {
        count.fetch_add(1, Ordering::SeqCst);
        if unsubscribe { HandlerResult::Unsubscribe } else { HandlerResult::Continue }
    });
}

#[tokio::test]
async fn event_subscribe_same_handler_type_twice() {
    let cx = new("").await;

    let a = Arc::new(AtomicUsize::new(0));
    let b = Arc::new(AtomicUsize::new(0));
    subscribe_counter::<SameHandlerTypeEvent>(a.clone(), false);
    subscribe_counter::<SameHandlerTypeEvent>(b.clone(), false);

    cx.with(|editor| event::dispatch(editor, SameHandlerTypeEvent)).await;

    assert_eq!(a.load(Ordering::SeqCst), 1);
    assert_eq!(b.load(Ordering::SeqCst), 1);

    cx.cleanup().await;
}

#[tokio::test]
async fn event_unsubscribe_removes_only_that_subscription() {
    let cx = new("").await;

    let a = Arc::new(AtomicUsize::new(0));
    let b = Arc::new(AtomicUsize::new(0));
    subscribe_counter::<UnsubscribeOneEvent>(a.clone(), true);
    subscribe_counter::<UnsubscribeOneEvent>(b.clone(), false);

    cx.with(|editor| event::dispatch(editor, UnsubscribeOneEvent)).await;
    subscribe_counter::<UnrelatedEvent>(Arc::new(AtomicUsize::new(0)), false);
    cx.with(|editor| event::dispatch(editor, UnsubscribeOneEvent)).await;

    assert_eq!(a.load(Ordering::SeqCst), 1);
    assert_eq!(b.load(Ordering::SeqCst), 2);

    cx.cleanup().await;
}

#[tokio::test]
async fn ready_input_is_rendered_in_one_batch() {
    let (mut editor, tasks) = zi::Editor::new(zi::DummyBackend, (80, 24));
    let inputs = "iabc".parse::<zi::input::KeySequence>().unwrap();
    let events = futures_util::stream::iter(inputs.into_iter().map(zi::input::Event::Key).map(Ok));
    let mut frames = Vec::new();

    editor
        .run(events, tasks, |editor| {
            let line = editor.cursor_line();
            if line == "abc" {
                editor.close_view(zi::Active);
            }
            frames.push(line);
            Ok(())
        })
        .await
        .unwrap();

    assert_eq!(frames, ["", "abc"]);
}
