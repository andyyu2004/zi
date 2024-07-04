use std::future::Future;

use futures_util::TryFutureExt;
use zi_lsp::lsp_types;

use super::{active_servers_of, Selector, State};
use crate::lsp::{from_proto, to_proto};
use crate::{Active, Editor, ViewId};

impl Editor {
    pub(crate) fn trigger_completion(&mut self) {
        let Some(fut) = self.request_completions() else { return };

        self.callback("completions", fut.map_err(Into::into), |editor, items| {
            let State::Insert(state) = &mut editor.state else { return Ok(()) };
            state.completion.show = true;
            state.completion.items = from_proto::completions(items).collect();

            return Ok(());
        });
    }

    fn request_completions(
        &mut self,
    ) -> Option<impl Future<Output = zi_lsp::Result<Vec<lsp_types::CompletionItem>>>> {
        let view: ViewId = Active.select(self);
        let buf = self[view].buffer();
        let Some((server, _caps)) = active_servers_of!(self, buf).find_map(|server| {
            let caps: lsp_types::CompletionOptions =
                self.active_language_servers[&server].capabilities.completion_provider.clone()?;
            Some((server, caps))
        }) else {
            tracing::warn!("No completion provider found for buffer {:?}", buf);
            return None;
        };

        let Some(uri) = self[buf].file_url().cloned() else { return None };

        let cursor = self[view].cursor();
        let s = self.active_language_servers.get_mut(&server).unwrap();
        let text = self.buffers[buf].text();
        let encoding = s.position_encoding();

        let fut = s.completion(lsp_types::CompletionParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: to_proto::point(encoding, &text, cursor),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        });

        Some(async move {
            let items = match fut.await? {
                Some(lsp_types::CompletionResponse::List(list)) => list.items,
                Some(lsp_types::CompletionResponse::Array(items)) => items,
                None => vec![],
            };

            Ok(items)
        })
    }
}
