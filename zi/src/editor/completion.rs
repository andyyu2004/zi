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
use crate::completion::{Completion, CompletionProvider};
use crate::{lstypes, Active, Editor, LanguageServiceId, Result, ViewId};

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

            Ok(())
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
                params: lstypes::CompletionParams,
            ) -> BoxFuture<'static, Result<lstypes::CompletionResponse>> {
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
                    .completion_capabilities()
                    .map(|_| Provider::Lsp(LspCompletionProvider { server }))
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
        let futs = providers
            .into_iter()
            .filter_map(|provider| {
                let url = self[buf].file_url().cloned()?;
                let params = lstypes::CompletionParams {
                    at: lstypes::TextDocumentPointParams { url, point },
                };
                Some(provider.completions(self, params))
            })
            .collect::<Vec<_>>();

        async move {
            stream::iter(futs)
                .buffered(16)
                .try_fold(vec![], |mut acc, res| async move {
                    acc.extend(res.items);
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
        params: lstypes::CompletionParams,
    ) -> BoxFuture<'static, Result<lstypes::CompletionResponse>> {
        editor.active_language_services.get_mut(&self.server).unwrap().completion(params)
    }
}
