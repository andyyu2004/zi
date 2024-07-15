use std::any::Any;
use std::sync::{Arc, OnceLock};

use async_lsp::{lsp_types, LanguageServer};
use futures_util::future::BoxFuture;
use futures_util::{FutureExt, TryFutureExt};
use zi::{lstypes, LanguageService, LanguageServiceId, PositionEncoding};
use zi_event::{event, HandlerResult};

/// async_lsp::LanguageServer -> zi::LanguageService
pub struct ToLanguageService<S> {
    service_id: LanguageServiceId,
    server: S,
    capabilities: Arc<OnceLock<lsp_types::ServerCapabilities>>,
    position_encoding: OnceLock<PositionEncoding>,
    last_version_sync: Option<i32>,
}

impl<S> ToLanguageService<S> {
    pub fn new(service_id: LanguageServiceId, server: S) -> Self {
        Self {
            service_id,
            server,
            capabilities: Default::default(),
            position_encoding: Default::default(),
            last_version_sync: None,
        }
    }
}

type ResponseFuture<T> = BoxFuture<'static, zi::Result<T>>;

impl<S> LanguageService for ToLanguageService<S>
where
    S: LanguageServer<NotifyResult = async_lsp::Result<()>, Error = async_lsp::Error>
        + Send
        + Sync
        + 'static,
{
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
            let Some(uri) = editor[buf].file_url() else { return HandlerResult::Continue };
            let params = lsp_types::DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri: uri.clone(),
                    language_id: editor[buf].file_type().to_string(),
                    version: editor[buf].version() as i32,
                    text: editor[buf].text().to_string(),
                },
            };

            // TODO should ignore any open events not related to this language server.
            // See below
            //

            if let Some(server) = editor.language_service(service_id) {
                tracing::debug!(?event, ?service_id, "lsp buffer did open");
                let server = downcast::<S>(server);
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

                let service = downcast::<S>(&mut **service);

                let encoding = service.position_encoding();

                let kind = match &service.capabilities().text_document_sync {
                    Some(cap) => match cap {
                        lsp_types::TextDocumentSyncCapability::Kind(kind) => kind,
                        lsp_types::TextDocumentSyncCapability::Options(opts) => {
                            match &opts.change {
                                Some(kind) => kind,
                                None => return HandlerResult::Continue,
                            }
                        }
                    },
                    None => return HandlerResult::Continue,
                };

                tracing::debug!(%uri, ?service_id, "lsp did_change");
                let version = buf.version() as i32;
                let text_document = lsp_types::VersionedTextDocumentIdentifier { uri, version };

                let content_changes = match *kind {
                    lsp_types::TextDocumentSyncKind::INCREMENTAL => {
                        if service.last_version_sync == version.checked_sub(1) {
                            zi::lsp::to_proto::deltas(
                                encoding,
                                event.old_text.as_ref(),
                                &event.deltas,
                            )
                        } else {
                            // If a version is skipped somehow, send the full text.
                            vec![lsp_types::TextDocumentContentChangeEvent {
                                range: None,
                                range_length: None,
                                text: buf.text().to_string(),
                            }]
                        }
                    }
                    lsp_types::TextDocumentSyncKind::FULL => {
                        vec![lsp_types::TextDocumentContentChangeEvent {
                            range: None,
                            range_length: None,
                            text: buf.text().to_string(),
                        }]
                    }
                    lsp_types::TextDocumentSyncKind::NONE => return HandlerResult::Continue,
                    _ => unreachable!("invalid text document sync kind: {kind:?}"),
                };

                match service.server.did_change(lsp_types::DidChangeTextDocumentParams {
                    text_document,
                    content_changes,
                }) {
                    Ok(_) => service.last_version_sync = Some(version),
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
    ) -> ResponseFuture<Option<Vec<lsp_types::TextEdit>>> {
        self.server
            .formatting(lsp_types::DocumentFormattingParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: params.url },
                options: params.options,
                work_done_progress_params: Default::default(),
            })
            .map_err(Into::into)
            .boxed()
    }

    fn definition(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse> {
        self.server
            .definition(to_proto::goto_definition(params))
            .map(|res| res.map(from_proto::goto_definition))
            .map_err(Into::into)
            .boxed()
    }

    fn type_definition(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse> {
        self.server
            .type_definition(to_proto::goto_definition(params))
            .map(|res| res.map(from_proto::goto_definition))
            .map_err(Into::into)
            .boxed()
    }

    fn implementation(
        &mut self,
        params: lstypes::GotoDefinitionParams,
    ) -> ResponseFuture<lstypes::GotoDefinitionResponse> {
        self.server
            .implementation(to_proto::goto_definition(params))
            .map(|res| res.map(from_proto::goto_definition))
            .map_err(Into::into)
            .boxed()
    }

    fn references(
        &mut self,
        params: lstypes::ReferenceParams,
    ) -> ResponseFuture<Vec<lstypes::Location>> {
        self.server
            .references(lsp_types::ReferenceParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri: params.at.url },
                    position: params.at.point,
                },
                context: lsp_types::ReferenceContext { include_declaration: true },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .map(|res| {
                res.map(|locs| match locs {
                    None => vec![],
                    Some(locs) => locs
                        .into_iter()
                        .map(|loc| lstypes::Location { uri: loc.uri, range: loc.range })
                        .collect(),
                })
            })
            .map_err(Into::into)
            .boxed()
    }

    fn completion(
        &mut self,
        params: lsp_types::CompletionParams,
    ) -> ResponseFuture<Option<lsp_types::CompletionResponse>> {
        self.server.completion(params).map_err(Into::into).boxed()
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
        params: lsp_types::DocumentDiagnosticParams,
    ) -> ResponseFuture<lsp_types::DocumentDiagnosticReportResult> {
        self.server.document_diagnostic(params).map_err(Into::into).boxed()
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

fn downcast<S: 'static>(service: &mut dyn LanguageService) -> &mut ToLanguageService<S> {
    service.as_any_mut().downcast_mut::<ToLanguageService<S>>().expect("failed to downcast")
}

mod from_proto {
    use async_lsp::lsp_types;
    use zi::lstypes;

    pub fn goto_definition(
        res: Option<lsp_types::GotoDefinitionResponse>,
    ) -> lstypes::GotoDefinitionResponse {
        match res {
            None => lstypes::GotoDefinitionResponse::Array(vec![]),
            Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) => {
                lstypes::GotoDefinitionResponse::Array(vec![location(loc)])
            }
            Some(lsp_types::GotoDefinitionResponse::Array(locs)) => {
                lstypes::GotoDefinitionResponse::Array(locs.into_iter().map(location).collect())
            }
            Some(lsp_types::GotoDefinitionResponse::Link(links)) => {
                lstypes::GotoDefinitionResponse::Array(
                    links
                        .into_iter()
                        .map(|link| lstypes::Location {
                            uri: link.target_uri,
                            range: link.target_selection_range,
                        })
                        .collect(),
                )
            }
        }
    }

    pub fn location(loc: lsp_types::Location) -> lstypes::Location {
        lstypes::Location { uri: loc.uri, range: loc.range }
    }
}

mod to_proto {
    use async_lsp::lsp_types;
    use zi::lstypes;

    pub fn goto_definition(
        params: lstypes::GotoDefinitionParams,
    ) -> lsp_types::GotoDefinitionParams {
        lsp_types::GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: params.at.url },
                position: params.at.point,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }
    }
}
