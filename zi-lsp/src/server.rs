use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use async_lsp::lsp_types;
use futures_util::future::BoxFuture;
use futures_util::{FutureExt, TryFutureExt};
use zi::{
    lstypes, LanguageServiceId, PositionEncoding, Resource, Rope, Text, TextMut, TextSlice, Url,
};
use zi_event::{event, HandlerResult};

use crate::{from_proto, to_proto, EditorExt};

/// async_lsp::LanguageServer -> zi::LanguageService
// We box the inner server instead of making it generic to make downcasting to this type possible.
pub struct LanguageService {
    service_id: LanguageServiceId,
    server: Box<
        dyn async_lsp::LanguageServer<
                NotifyResult = async_lsp::Result<()>,
                Error = async_lsp::Error,
            > + Send
            + Sync,
    >,
    capabilities: Arc<OnceLock<lsp_types::ServerCapabilities>>,
    position_encoding: OnceLock<PositionEncoding>,
    // Keeping track of this here for encoding conversions (and sanity checks)
    texts: HashMap<Url, (i32, Rope)>,
}

impl LanguageService {
    pub fn new(
        service_id: LanguageServiceId,
        server: impl async_lsp::LanguageServer<
            NotifyResult = async_lsp::Result<()>,
            Error = async_lsp::Error,
        > + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            service_id,
            server: Box::new(server),
            capabilities: Default::default(),
            position_encoding: Default::default(),
            texts: Default::default(),
        }
    }

    #[inline]
    pub(crate) fn text(&self, url: &Url) -> Option<&Rope> {
        let (_, text) = self.texts.get(url)?;
        Some(text)
    }
}

type ResponseFuture<T> = BoxFuture<'static, zi::Result<T>>;

impl zi::LanguageService for LanguageService {
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn initialize(&mut self, params: lstypes::InitializeParams) -> ResponseFuture<()> {
        let caps = Arc::clone(&self.capabilities);
        let fut = self.server.initialize(lsp_types::InitializeParams {
            process_id: Some(params.process_id),
            capabilities: params.capabilities,
            workspace_folders: Some(params.workspace_folders),
            ..Default::default()
        });
        Box::pin(async move {
            let result = fut.await?;
            caps.set(result.capabilities).expect("capabilities already initialized");
            Ok(())
        })
    }

    fn initialized(&mut self) -> zi::Result<()> {
        self.server.initialized(lsp_types::InitializedParams {})?;

        // Setup relevant event handlers to create notifications to the language server.
        let service_id = self.service_id;

        zi::event::subscribe_with::<event::DidOpenBuffer>(move |editor, event| {
            let buf = event.buf;
            let url = editor[buf].url().clone();
            let params = lsp_types::DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri: url.clone(),
                    language_id: editor[buf].file_type().to_string(),
                    version: editor[buf].version() as i32,
                    text: editor[buf].text().to_string(),
                },
            };

            // TODO should ignore any open events not related to this language server.
            // See below

            if let Some(server) = editor.language_server(service_id) {
                tracing::debug!(?event, ?service_id, "lsp buffer did open");
                server.texts.entry(url).or_insert((
                    params.text_document.version,
                    Rope::from(params.text_document.text.as_str()),
                ));

                if let Err(err) = server.server.did_open(params) {
                    tracing::error!(?err, "lsp did_open notification failed");
                }
            }

            HandlerResult::Continue
        });

        zi::event::subscribe_with::<event::DidChangeBuffer>(move |editor, event| {
            tracing::trace!(buf = ?event.buf, "buffer did change");

            let buf = &editor.buffers[event.buf];
            if let (Some(service), uri) =
                (editor.active_language_services.get_mut(&service_id), buf.url().clone())
            {
                if !editor
                    .language_config
                    .languages
                    .get(&buf.file_type())
                    .map(|c| &c.language_services)
                    .map_or(false, |servers| servers.contains(&service_id))
                {
                    return HandlerResult::Continue;
                }

                let service = crate::downcast_mut(&mut **service);

                let encoding = service.position_encoding();

                let kind = match service.capabilities().text_document_sync.clone() {
                    Some(cap) => match cap {
                        lsp_types::TextDocumentSyncCapability::Kind(kind) => kind,
                        lsp_types::TextDocumentSyncCapability::Options(opts) => match opts.change {
                            Some(kind) => kind,
                            None => return HandlerResult::Continue,
                        },
                    },
                    None => return HandlerResult::Continue,
                };

                tracing::debug!(%uri, ?service_id, "lsp did_change");
                let version = buf.version() as i32;
                let text_document = lsp_types::VersionedTextDocumentIdentifier { uri, version };

                let (text_version, text) =
                    service.texts.get_mut(&text_document.uri).expect("buffer not opened");
                let content_changes = match kind {
                    lsp_types::TextDocumentSyncKind::INCREMENTAL => {
                        if version.checked_sub(1) == Some(*text_version) {
                            debug_assert_eq!(
                                *text,
                                event.old_text.to_string(),
                                "lsp text desynced"
                            );
                            text.edit(&event.deltas);
                            to_proto::deltas(encoding, event.old_text.as_ref(), &event.deltas)
                        } else {
                            let mut builder = zi::RopeBuilder::new();
                            for chunk in buf.text().byte_slice(..).chunks() {
                                builder.append(chunk);
                            }
                            *text = builder.build();
                            // If a version is skipped somehow, send the full text.
                            vec![lsp_types::TextDocumentContentChangeEvent {
                                range: None,
                                range_length: None,
                                text: buf.text().to_string(),
                            }]
                        }
                    }
                    lsp_types::TextDocumentSyncKind::FULL => {
                        let mut builder = zi::RopeBuilder::new();
                        for chunk in buf.text().byte_slice(..).chunks() {
                            builder.append(chunk);
                        }
                        *text = builder.build();
                        vec![lsp_types::TextDocumentContentChangeEvent {
                            range: None,
                            range_length: None,
                            text: buf.text().to_string(),
                        }]
                    }
                    lsp_types::TextDocumentSyncKind::NONE => return HandlerResult::Continue,
                    _ => unreachable!("invalid text document sync kind: {kind:?}"),
                };

                debug_assert_eq!(*text, buf.text().to_string(), "lsp text desynced");
                match service.server.did_change(lsp_types::DidChangeTextDocumentParams {
                    text_document,
                    content_changes,
                }) {
                    Ok(_) => *text_version = version,
                    Err(err) => tracing::error!(?err, "lsp did_change notification failed"),
                }
            }

            HandlerResult::Continue
        });

        Ok(())
    }

    fn formatting(
        &mut self,
        params: lstypes::DocumentFormattingParams,
    ) -> ResponseFuture<Option<zi::Deltas<'static>>> {
        let enc = self.position_encoding();
        let (_, text) = self.texts.get(&params.url).unwrap().clone();
        self.server
            .formatting(lsp_types::DocumentFormattingParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: params.url },
                options: params.options,
                work_done_progress_params: Default::default(),
            })
            .map(move |res| {
                res.map(|opt| opt.and_then(|edits| from_proto::deltas(enc, &text, edits)))
            })
            .map_err(Into::into)
            .boxed()
    }

    fn definition(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse> {
        let enc = self.position_encoding();
        let (_, text) = self.texts.get(&params.at.url).unwrap().clone();
        self.server
            .definition(to_proto::goto_definition(enc, &text, params))
            .map(move |res| match res {
                Ok(res) => match from_proto::goto_definition(enc, &text, res) {
                    Some(res) => Ok(res),
                    None => Ok(lstypes::GotoDefinitionResponse::Array(vec![])),
                },
                Err(err) => Err(err),
            })
            .map_err(Into::into)
            .boxed()
    }

    fn type_definition(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse> {
        let enc = self.position_encoding();
        let (_, text) = self.texts.get(&params.at.url).unwrap().clone();
        self.server
            .type_definition(to_proto::goto_definition(enc, &text, params))
            .map(move |res| match res {
                Ok(res) => match from_proto::goto_definition(enc, &text, res) {
                    Some(res) => Ok(res),
                    None => Ok(lstypes::GotoDefinitionResponse::Array(vec![])),
                },
                Err(err) => Err(err),
            })
            .map_err(Into::into)
            .boxed()
    }

    fn implementation(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse> {
        let enc = self.position_encoding();
        let (_, text) = self.texts.get(&params.at.url).unwrap().clone();
        self.server
            .implementation(to_proto::goto_definition(enc, &text, params))
            .map(move |res| match res {
                Ok(res) => match from_proto::goto_definition(enc, &text, res) {
                    Some(res) => Ok(res),
                    None => Ok(lstypes::GotoDefinitionResponse::Array(vec![])),
                },
                Err(err) => Err(err),
            })
            .map_err(Into::into)
            .boxed()
    }

    fn references(
        &mut self,
        params: lstypes::ReferenceParams,
    ) -> ResponseFuture<Vec<lstypes::Location>> {
        let enc = self.position_encoding();
        let (_, text) = self.texts.get(&params.at.url).unwrap().clone();
        self.server
            .references(lsp_types::ReferenceParams {
                text_document_position: to_proto::document_position(enc, &text, params.at),
                context: lsp_types::ReferenceContext { include_declaration: true },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .map(move |res| {
                res.map(|locs| match locs {
                    None => vec![],
                    Some(locs) => locs
                        .into_iter()
                        .filter_map(|loc| from_proto::location(enc, &text, loc))
                        .collect(),
                })
            })
            .map_err(Into::into)
            .boxed()
    }

    fn completion(
        &mut self,
        params: lstypes::CompletionParams,
    ) -> ResponseFuture<lstypes::CompletionResponse> {
        let enc = self.position_encoding();
        let (_, text) = self.texts.get(&params.at.url).unwrap().clone();
        self.server
            .completion(lsp_types::CompletionParams {
                text_document_position: to_proto::document_position(enc, &text, params.at),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: None,
            })
            .map(move |res| {
                res.map(|opt| {
                    opt.map_or_else(Default::default, |res| {
                        from_proto::completion_response(enc, &text, res)
                    })
                })
            })
            .map_err(Into::into)
            .boxed()
    }

    fn semantic_tokens_full(
        &mut self,
        params: lsp_types::SemanticTokensParams,
    ) -> ResponseFuture<Option<lsp_types::SemanticTokensResult>> {
        self.server.semantic_tokens_full(params).map_err(Into::into).boxed()
    }

    fn semantic_tokens_full_delta(
        &mut self,
        params: lsp_types::SemanticTokensDeltaParams,
    ) -> ResponseFuture<Option<lsp_types::SemanticTokensFullDeltaResult>> {
        self.server.semantic_tokens_full_delta(params).map_err(Into::into).boxed()
    }

    fn document_diagnostic(
        &mut self,
        params: lstypes::DocumentDiagnosticParams,
    ) -> ResponseFuture<lstypes::DocumentDiagnosticReport> {
        let enc = self.position_encoding();
        let (_version, text) = self.texts.get(&params.url).unwrap().clone();
        self.server
            .document_diagnostic(lsp_types::DocumentDiagnosticParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: params.url },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .map(move |res| {
                res.map(move |res| match res {
                    lsp_types::DocumentDiagnosticReportResult::Report(res) => match res {
                        lsp_types::DocumentDiagnosticReport::Full(res) => {
                            lstypes::DocumentDiagnosticReport {
                                diagnostics: from_proto::diagnostics(
                                    enc,
                                    &text,
                                    res.full_document_diagnostic_report.items,
                                ),
                                // Need to open any unopened buffers and get the text to do the conversions for related documents
                                related_documents: Default::default(),
                            }
                        }
                        lsp_types::DocumentDiagnosticReport::Unchanged(_) => {
                            lstypes::DocumentDiagnosticReport::default()
                        }
                    },
                    lsp_types::DocumentDiagnosticReportResult::Partial(_res) => {
                        // ditto: related documents unimplemented
                        lstypes::DocumentDiagnosticReport::default()
                    }
                })
            })
            .map_err(Into::into)
            .boxed()
    }

    fn capabilities(&self) -> &lsp_types::ServerCapabilities {
        self.capabilities.get().expect("capabilities not initialized")
    }

    fn position_encoding(&self) -> PositionEncoding {
        *self.position_encoding.get_or_init(|| match &Self::capabilities(self).position_encoding {
            Some(encoding) => match encoding {
                enc if *enc == lsp_types::PositionEncodingKind::UTF8 => PositionEncoding::Utf8,
                enc if *enc == lsp_types::PositionEncodingKind::UTF16 => PositionEncoding::Utf16,
                _ => {
                    tracing::warn!("server returned unknown position encoding: {encoding:?}",);
                    PositionEncoding::default()
                }
            },
            None => {
                tracing::warn!("server did not return position encoding, defaulting to UTF-16");
                PositionEncoding::default()
            }
        })
    }

    fn shutdown(&mut self) -> ResponseFuture<()> {
        self.server.shutdown(()).map_err(Into::into).boxed()
    }

    fn exit(&mut self) -> anyhow::Result<()> {
        self.server.exit(())?;
        Ok(())
    }
}
