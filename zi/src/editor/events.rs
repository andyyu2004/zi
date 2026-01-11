use super::*;
use crate::Editor;
use crate::event::{self, HandlerResult};

mod lsp;

impl Editor {
    pub(super) fn subscribe_sync_hooks() {
        event::subscribe(Self::lsp_did_open_refresh_semantic_tokens());

        event::subscribe_with::<event::DidSaveBuffer>(|editor, event| {
            editor.refresh_semantic_tokens(event.buf);
            HandlerResult::Continue
        });

        event::subscribe_with::<event::WillChangeMode>(|editor, event| {
            if let (Mode::Insert, Mode::Normal) = (event.from, event.to) {
                editor.insert_to_normal()
            }

            HandlerResult::Continue
        });

        event::subscribe_with::<event::DidChangeMode>(|editor, event| {
            if let (Mode::Insert, Mode::Normal) = (event.from, event.to) {
                editor.refresh_semantic_tokens(Active.select(editor))
            }

            // Handle dot repeat recording based on mode transitions
            if !editor.dot.is_replaying() {
                if Dot::should_start_recording(event.from, event.to) {
                    editor.dot.start_recording();
                } else if Dot::should_stop_recording(event.from, event.to) {
                    editor.dot.stop_recording();
                }
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

        // Detect normal mode changes for dot repeat
        event::subscribe_with::<event::DidChangeBuffer>(|editor, _event| {
            if editor.mode() == Mode::Normal && !editor.dot.is_replaying() {
                editor.dot.finalize_normal_mode_change();
            }

            HandlerResult::Continue
        });
    }

    pub(super) async fn subscribe_async_hooks() {
        event::subscribe_async(Self::format_before_save()).await;
    }
}
