use std::any::TypeId;
use std::future::Future;
use std::sync::{Arc, OnceLock};

use futures_core::future::BoxFuture;
use futures_util::{stream, StreamExt, TryFutureExt, TryStreamExt};
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use zi_core::CompletionItem;
use zi_text::{Delta, Deltas};

use super::{active_servers_of, Selector, State};
use crate::completion::{Completion, CompletionParams, CompletionProvider};
use crate::lsp::{from_proto, to_proto};
use crate::{Active, Editor, LanguageServiceId, Result, ViewId};

static COMPLETION_PROVIDERS: OnceLock<RwLock<FxHashMap<TypeId, Arc<dyn CompletionProvider>>>> =
    OnceLock::new();

impl Editor {
    pub fn register_completion_provider<P: CompletionProvider + 'static>(&mut self, provider: P) {
        COMPLETION_PROVIDERS
            .get_or_init(Default::default)
            .write()
            .insert(TypeId::of::<P>(), Arc::new(provider));
    }

    pub fn unregister_completion_provider<P: CompletionProvider + 'static>(&mut self) {
        COMPLETION_PROVIDERS.get_or_init(Default::default).write().remove(&TypeId::of::<P>());
    }

    pub fn completions(&self) -> Option<impl ExactSizeIterator<Item = &CompletionItem>> {
        match &self.state {
            State::Insert(state) => Some(state.completion.matches()),
            _ => None,
        }
    }

    pub fn trigger_completion(&mut self, trigger: Option<char>) {
        let fut = self.request_completions(Active);
        let at = self.cursor_byte(Active);

        let State::Insert(state) = &mut self.state else { return };
        state.completion.activate(at, trigger);

        self.callback("completions", fut.map_err(Into::into), move |editor, items| {
            let State::Insert(state) = &mut editor.state else { return Ok(()) };
            if let Completion::Active(state) = &mut state.completion {
                state.set_items(items);
            }

            return Ok(());
        });
    }

    pub(super) fn apply_completion_delta(&mut self, delta: Delta<'_>) {
        let delta = delta.to_owned();
        let new_cursor = delta.range().start + delta.text().len();
        self.edit(Active, &Deltas::new([delta])).expect("valid delta");
        self.set_cursor_bytewise(Active, new_cursor);
    }

    #[doc(hidden)]
    pub fn request_completions(
        &mut self,
        view: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<Vec<CompletionItem>>> {
        enum Provider {
            Lsp(LspCompletionProvider),
            Provider(Arc<dyn CompletionProvider>),
        }

        impl CompletionProvider for Provider {
            fn completions(
                &self,
                editor: &mut Editor,
                params: CompletionParams,
            ) -> BoxFuture<'static, Result<Vec<CompletionItem>>> {
                match self {
                    Provider::Lsp(provider) => provider.completions(editor, params),
                    Provider::Provider(provider) => provider.completions(editor, params),
                }
            }
        }

        let view = view.select(self);
        let buf = self[view].buffer();
        let providers = active_servers_of!(self, buf)
            .filter_map(|&server| {
                self.active_language_services[&server]
                    .capabilities()
                    .completion_provider
                    .clone()?;
                Some(Provider::Lsp(LspCompletionProvider { server }))
            })
            .chain(
                COMPLETION_PROVIDERS
                    .get_or_init(Default::default)
                    .read()
                    .values()
                    .map(|provider| Provider::Provider(Arc::clone(provider))),
            )
            .collect::<Vec<_>>();

        let point = self[view].cursor();
        let params = CompletionParams { buf, point };
        let futs = providers
            .into_iter()
            .map(|provider| provider.completions(self, params))
            .collect::<Vec<_>>();

        async move {
            stream::iter(futs)
                .buffered(16)
                .try_fold(vec![], |mut acc, items| async move {
                    acc.extend(items);
                    Ok(acc)
                })
                .await
        }
    }
}

struct LspCompletionProvider {
    server: LanguageServiceId,
}

impl CompletionProvider for LspCompletionProvider {
    fn completions(
        &self,
        editor: &mut Editor,
        params: CompletionParams,
    ) -> BoxFuture<'static, Result<Vec<CompletionItem>>> {
        let buf = params.buf;
        let Some(uri) = editor[buf].file_url().cloned() else {
            return Box::pin(async move { Ok(vec![]) });
        };
        let s = editor.active_language_services.get_mut(&self.server).unwrap();
        let text = editor.buffers[buf].text();
        let encoding = s.position_encoding();

        let fut = s.completion(lsp_types::CompletionParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: to_proto::point(encoding, &text, params.point),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        });

        Box::pin(async {
            let items = match fut.await? {
                Some(lsp_types::CompletionResponse::List(list)) => list.items,
                Some(lsp_types::CompletionResponse::Array(items)) => items,
                None => vec![],
            };

            Ok(from_proto::completions(items).collect())
        })
    }
}
