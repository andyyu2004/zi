use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use async_lsp::lsp_types::{self, OneOf};
use futures_util::future::BoxFuture;
use futures_util::{FutureExt, TryFutureExt};
use zi::{event, lstypes, LanguageServiceId, Rope, Setting, Text, TextMut, TextSlice, Theme, Url};
use zi_event::HandlerResult;

use crate::{client, from_proto, to_proto, EditorExt};

/// async_lsp::LanguageServer -> zi::LanguageService
// We box the inner server instead of making it generic to make downcasting to this type possible.
pub struct LanguageService {
    _client: zi::LanguageClient,
    service_id: LanguageServiceId,
    server: Box<
        dyn async_lsp::LanguageServer<
                NotifyResult = async_lsp::Result<()>,
                Error = async_lsp::Error,
            > + Send
            + Sync,
    >,
    capabilities: Arc<OnceLock<lsp_types::ServerCapabilities>>,
    position_encoding: OnceLock<lstypes::PositionEncoding>,
    // Keeping track of this here for encoding conversions (and sanity checks)
    texts: HashMap<Url, (i32, Rope)>,
    semantic_tokens_legend: OnceLock<Option<Arc<lsp_types::SemanticTokensLegend>>>,
}

impl LanguageService {
    pub fn new(
        client: zi::LanguageClient,
        server: impl async_lsp::LanguageServer<
            NotifyResult = async_lsp::Result<()>,
            Error = async_lsp::Error,
        > + Send
        + Sync
        + 'static,
    ) -> Self {
        let service_id = client.service_id();
        Self {
            _client: client,
            service_id,
            server: Box::new(server),
            capabilities: Default::default(),
            position_encoding: Default::default(),
            texts: Default::default(),
            semantic_tokens_legend: Default::default(),
        }
    }

    #[inline]
    pub(crate) fn text(&self, url: &Url) -> Option<&Rope> {
        let (_, text) = self.texts.get(url)?;
        Some(text)
    }

    fn semantic_tokens_legend(&self) -> Option<Arc<lsp_types::SemanticTokensLegend>> {
        self.semantic_tokens_legend
            .get_or_init(|| {
                let opts = match self.capabilities().semantic_tokens_provider.as_ref()? {
                    lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(opts) => opts,
                    lsp_types::SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(opts) => &opts.semantic_tokens_options,
                };

                Some(Arc::new(opts.legend.clone()))
            })
            .clone()
    }

    pub(crate) fn position_encoding(&self) -> lstypes::PositionEncoding {
        *self.position_encoding.get_or_init(|| match &Self::capabilities(self).position_encoding {
            Some(encoding) => match encoding {
                enc if *enc == lsp_types::PositionEncodingKind::UTF8 => {
                    lstypes::PositionEncoding::Utf8
                }
                enc if *enc == lsp_types::PositionEncodingKind::UTF16 => {
                    lstypes::PositionEncoding::Utf16
                }
                _ => {
                    tracing::warn!("server returned unknown position encoding: {encoding:?}",);
                    lstypes::PositionEncoding::default()
                }
            },
            None => {
                tracing::warn!("server did not return position encoding, defaulting to UTF-16");
                lstypes::PositionEncoding::default()
            }
        })
    }

    fn capabilities(&self) -> &lsp_types::ServerCapabilities {
        self.capabilities.get().expect("capabilities not initialized")
    }
}

type ResponseFuture<T> = BoxFuture<'static, zi::Result<T>>;

impl zi::LanguageService for LanguageService {
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn definition_capabilities(&self) -> Option<()> {
        matches!(self.capabilities().definition_provider, Some(OneOf::Left(true) | OneOf::Right(_)))
            .then_some(())
    }

    fn declaration_capabilities(&self) -> Option<()> {
        (!matches!(
            self.capabilities().declaration_provider,
            None | Some(lsp_types::DeclarationCapability::Simple(false))
        ))
        .then_some(())
    }

    fn implementation_capabilities(&self) -> Option<()> {
        (!matches!(
            self.capabilities().implementation_provider,
            None | Some(lsp_types::ImplementationProviderCapability::Simple(false))
        ))
        .then_some(())
    }

    fn type_definition_capabilities(&self) -> Option<()> {
        (!matches!(
            self.capabilities().type_definition_provider,
            None | Some(lsp_types::TypeDefinitionProviderCapability::Simple(false))
        ))
        .then_some(())
    }

    fn completion_capabilities(&self) -> Option<()> {
        self.capabilities().completion_provider.as_ref()?;
        Some(())
    }

    fn reference_capabilities(&self) -> Option<()> {
        self.capabilities().references_provider.as_ref()?;
        Some(())
    }

    fn diagnostic_capabilities(&self) -> Option<()> {
        self.capabilities().diagnostic_provider.as_ref()?;
        Some(())
    }

    fn semantic_tokens_capabilities(&self) -> Option<()> {
        let caps = self.capabilities().semantic_tokens_provider.as_ref()?;

        let caps = match caps {
            lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(opts) => opts,
            lsp_types::SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                opts,
            ) => &opts.semantic_tokens_options,
        };

        // Don't care if the server supports only range, we don't use it.
        caps.full.as_ref()?;
        Some(())
    }

    fn formatting_capabilities(&self) -> Option<()> {
        self.capabilities().document_formatting_provider.as_ref()?;
        Some(())
    }

    fn initialize(&mut self, params: lstypes::InitializeParams) -> ResponseFuture<()> {
        let caps = Arc::clone(&self.capabilities);
        let fut = self.server.initialize(lsp_types::InitializeParams {
            process_id: Some(params.process_id),
            capabilities: client::capabilities(),
            workspace_folders: Some(
                params
                    .workspace_folders
                    .into_iter()
                    .map(|f| lsp_types::WorkspaceFolder { uri: f.uri, name: f.name })
                    .collect(),
            ),
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
            let Some(url) = editor[buf].file_url().cloned() else { return HandlerResult::Continue };
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
            if let (Some(service), Some(uri)) =
                (editor.active_language_services.get_mut(&service_id), buf.file_url().cloned())
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

                fn build_rope(text: impl Text) -> Rope {
                    let mut builder = zi::RopeBuilder::new();
                    for chunk in text.byte_slice(..).chunks() {
                        builder.append(chunk);
                    }
                    builder.build()
                }

                let content_changes = match kind {
                    lsp_types::TextDocumentSyncKind::INCREMENTAL
                    | lsp_types::TextDocumentSyncKind::FULL => {
                        let (prev_version, text) = service
                            .texts
                            .entry(text_document.uri.clone())
                            .or_insert_with(|| (0, Rope::new()));

                        let content_changes = if kind
                            == lsp_types::TextDocumentSyncKind::INCREMENTAL
                            && *prev_version > 0
                            && version.checked_sub(1) == Some(*prev_version)
                        {
                            // If has incremental support and versions are consecutive, send deltas.
                            debug_assert_eq!(
                                *text,
                                event.old_text.to_string(),
                                "old lsp text desynced"
                            );
                            text.edit(&event.deltas);
                            debug_assert_eq!(
                                *text,
                                buf.text().to_string(),
                                "new lsp text desynced"
                            );
                            to_proto::deltas(encoding, event.old_text.as_ref(), &event.deltas)
                        } else {
                            *text = build_rope(buf.text());
                            // If a version is skipped somehow, send the full text.
                            vec![lsp_types::TextDocumentContentChangeEvent {
                                range: None,
                                range_length: None,
                                text: buf.text().to_string(),
                            }]
                        };
                        *prev_version = version;
                        content_changes
                    }
                    lsp_types::TextDocumentSyncKind::NONE => return HandlerResult::Continue,
                    _ => unreachable!("invalid text document sync kind: {kind:?}"),
                };

                if let Err(err) =
                    service.server.did_change(lsp_types::DidChangeTextDocumentParams {
                        text_document,
                        content_changes,
                    })
                {
                    tracing::error!(?err, "lsp did_change notification failed")
                }
            }

            HandlerResult::Continue
        });

        Ok(())
    }

    fn format(
        &mut self,
        params: lstypes::DocumentFormattingParams,
    ) -> ResponseFuture<Option<zi::Deltas<'static>>> {
        let enc = self.position_encoding();
        let Some(text) = self.text(&params.url).cloned() else {
            return Box::pin(async { Ok(None) });
        };

        self.server
            .formatting(lsp_types::DocumentFormattingParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: params.url },
                options: lsp_types::FormattingOptions {
                    tab_size: params.options.tab_size,
                    insert_spaces: true,
                    trim_trailing_whitespace: Some(true),
                    insert_final_newline: Some(true),
                    trim_final_newlines: Some(true),
                    properties: Default::default(),
                },
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
        let Some(text) = self.text(&params.at.url).cloned() else {
            return Box::pin(async { Ok(Default::default()) });
        };

        self.server
            .definition(to_proto::goto_definition(enc, &text, params))
            .map(move |res| match res {
                Ok(res) => match from_proto::goto_definition(enc, res) {
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
        let Some(text) = self.text(&params.at.url).cloned() else {
            return Box::pin(async { Ok(Default::default()) });
        };
        self.server
            .type_definition(to_proto::goto_definition(enc, &text, params))
            .map(move |res| match res {
                Ok(res) => match from_proto::goto_definition(enc, res) {
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
        let Some(text) = self.text(&params.at.url).cloned() else {
            return Box::pin(async { Ok(Default::default()) });
        };
        self.server
            .implementation(to_proto::goto_definition(enc, &text, params))
            .map(move |res| match res {
                Ok(res) => match from_proto::goto_definition(enc, res) {
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
        let Some(text) = self.text(&params.at.url).cloned() else {
            return Box::pin(async { Ok(vec![]) });
        };
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
                    Some(locs) => {
                        locs.into_iter().filter_map(|loc| from_proto::location(enc, loc)).collect()
                    }
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
        let Some(text) = self.text(&params.at.url).cloned() else {
            return Box::pin(async { Ok(Default::default()) });
        };

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
        theme: Setting<Theme>,
        params: lstypes::SemanticTokensParams,
    ) -> ResponseFuture<Option<Vec<zi::MarkBuilder>>> {
        let encoding = self.position_encoding();
        let Some(text) = self.text(&params.url).cloned() else {
            return Box::pin(async { Ok(None) });
        };
        let legend = self
            .semantic_tokens_legend()
            .expect("should not request semantic tokens to service without the capability");

        self.server
            .semantic_tokens_full(lsp_types::SemanticTokensParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: params.url },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .map(move |res| {
                let theme = theme.read();
                match res {
                    Ok(Some(res)) => match res {
                        lsp_types::SemanticTokensResult::Tokens(tokens) => Ok(Some(
                            from_proto::semantic_tokens(encoding, &text, &legend, &theme, tokens),
                        )),
                        lsp_types::SemanticTokensResult::Partial(_tokens) => Ok(None),
                    },
                    Ok(None) => Ok(None),
                    Err(err) => Err(err),
                }
            })
            .map_err(Into::into)
            .boxed()
    }

    fn document_diagnostic(
        &mut self,
        params: lstypes::DocumentDiagnosticParams,
    ) -> ResponseFuture<lstypes::DocumentDiagnosticReport> {
        let enc = self.position_encoding();

        fn convert_related(
            enc: lstypes::PositionEncoding,
            kind: Option<HashMap<Url, lsp_types::DocumentDiagnosticReportKind>>,
        ) -> HashMap<Url, lstypes::Diagnostics> {
            kind.map_or_else(Default::default, |kind| {
                kind.into_iter()
                    .filter_map(|(url, kind)| {
                        Some((
                            url,
                            match kind {
                                lsp_types::DocumentDiagnosticReportKind::Full(full) => {
                                    lstypes::Diagnostics::Full(from_proto::diagnostics(
                                        enc, full.items,
                                    ))
                                }
                                lsp_types::DocumentDiagnosticReportKind::Unchanged(_) => {
                                    lstypes::Diagnostics::Unchanged
                                }
                            },
                        ))
                    })
                    .collect()
            })
        }

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
                                diagnostics: lstypes::Diagnostics::Full(from_proto::diagnostics(
                                    enc,
                                    res.full_document_diagnostic_report.items,
                                )),
                                related_documents: convert_related(enc, res.related_documents),
                            }
                        }
                        lsp_types::DocumentDiagnosticReport::Unchanged(_) => {
                            lstypes::DocumentDiagnosticReport {
                                diagnostics: lstypes::Diagnostics::Unchanged,
                                related_documents: Default::default(),
                            }
                        }
                    },
                    lsp_types::DocumentDiagnosticReportResult::Partial(res) => {
                        lstypes::DocumentDiagnosticReport {
                            related_documents: convert_related(enc, res.related_documents),
                            diagnostics: lstypes::Diagnostics::Unchanged,
                        }
                    }
                })
            })
            .map_err(Into::into)
            .boxed()
    }

    fn shutdown(&mut self) -> ResponseFuture<()> {
        self.server.shutdown(()).map_err(Into::into).boxed()
    }

    fn exit(&mut self) -> anyhow::Result<()> {
        self.server.exit(())?;
        Ok(())
    }
}
