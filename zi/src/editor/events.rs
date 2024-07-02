use super::*;
use crate::{event, Editor};

mod lsp;

impl Editor {
    pub(super) fn subscribe_sync_hooks() {
        event::subscribe(Self::lsp_did_open_refresh_semantic_tokens());
        event::subscribe(Self::lsp_did_change_refresh_semantic_tokens());

        event::subscribe_with::<event::WillChangeMode>(|editor, event| {
            match (event.from, event.to) {
                (Mode::Insert, Mode::Normal) => editor.insert_to_normal(),
                _ => (),
            }
        });
    }

    pub(super) async fn subscribe_async_hooks() {
        event::subscribe_async(Self::format_before_save()).await;
    }
}
