use super::*;
use crate::event::{self, HandlerResult};
use crate::Editor;

mod lsp;

impl Editor {
    pub(super) fn subscribe_sync_hooks() {
        event::subscribe(Self::lsp_did_open_refresh_semantic_tokens());

        event::subscribe_with::<event::DidSaveBuffer>(|editor, event| {
            editor.refresh_semantic_tokens(event.buf);
            HandlerResult::Continue
        });

        event::subscribe_with::<event::WillChangeMode>(|editor, event| {
            match (event.from, event.to) {
                (Mode::Insert, Mode::Normal) => editor.insert_to_normal(),
                _ => (),
            }
            HandlerResult::Continue
        });

        event::subscribe_with::<event::DidChangeMode>(|editor, event| {
            match (event.from, event.to) {
                (Mode::Insert, Mode::Normal) => {
                    editor.refresh_semantic_tokens(Active.select(editor))
                }
                _ => (),
            }
            HandlerResult::Continue
        });

        event::subscribe_with::<event::DidInsertChar>(|editor, event| {
            if event.view != Active.select(editor) {
                return HandlerResult::Continue;
            }

            let State::Insert(state) = &mut editor.state else { return HandlerResult::Continue };

            match event.char {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => match &mut state.completion {
                    Completion::Active(state) => state.update_query(Some(event.char)),
                    Completion::Inactive => editor.trigger_completion(Some(event.char)),
                },
                '.' | ':' => {
                    state.completion.deactivate();
                    editor.trigger_completion(Some(event.char));
                }
                _ => state.completion.deactivate(),
            }

            HandlerResult::Continue
        });

        event::subscribe_with::<event::DidDeleteChar>(|editor, event| {
            if event.view != Active.select(editor) {
                return HandlerResult::Continue;
            }

            let State::Insert(state) = &mut editor.state else {
                return HandlerResult::Continue;
            };

            if let Completion::Active(state) = &mut state.completion {
                state.update_query(None)
            }

            HandlerResult::Continue
        });
    }

    pub(super) async fn subscribe_async_hooks() {
        event::subscribe_async(Self::format_before_save()).await;
    }
}
