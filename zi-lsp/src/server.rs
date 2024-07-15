use std::any::Any;
use std::sync::{Arc, OnceLock};

use async_lsp::{lsp_types, LanguageServer};
use futures_util::future::BoxFuture;
use futures_util::{FutureExt, TryFutureExt};
use zi::{
    lstypes, LanguageService, LanguageServiceId, PositionEncoding, Rope, Text, TextMut, TextSlice,
};
use zi_event::{event, HandlerResult};

/// async_lsp::LanguageServer -> zi::LanguageService
pub struct ToLanguageService<S> {
    service_id: LanguageServiceId,
    server: S,
    capabilities: Arc<OnceLock<lsp_types::ServerCapabilities>>,
    position_encoding: OnceLock<PositionEncoding>,
    text_version: Option<i32>,
    // Keeping track of this here for encoding conversions (and sanity checks)
    text: Rope,
}

impl<S> ToLanguageService<S> {
    pub fn new(service_id: LanguageServiceId, server: S) -> Self {
        Self {
            service_id,
            server,
            capabilities: Default::default(),
            position_encoding: Default::default(),
            text_version: None,
            text: Default::default(),
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
                server.text = zi::Rope::from(params.text_document.text.as_str());
                server.text_version = Some(params.text_document.version);
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
                        if service.text_version == version.checked_sub(1) {
                            debug_assert_eq!(
                                event.old_text.to_string(),
                                service.text,
                                "lsp text desynced"
                            );
                            service.text.edit(&event.deltas);
                            zi::lsp::to_proto::deltas(
                                encoding,
                                event.old_text.as_ref(),
                                &event.deltas,
                            )
                        } else {
                            let mut builder = zi::RopeBuilder::new();
                            for chunk in buf.text().byte_slice(..).chunks() {
                                builder.append(chunk);
                            }
                            service.text = builder.build();
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
                        service.text = builder.build();
                        vec![lsp_types::TextDocumentContentChangeEvent {
                            range: None,
                            range_length: None,
                            text: buf.text().to_string(),
                        }]
                    }
                    lsp_types::TextDocumentSyncKind::NONE => return HandlerResult::Continue,
                    _ => unreachable!("invalid text document sync kind: {kind:?}"),
                };

                debug_assert_eq!(service.text, buf.text().to_string(), "lsp text desynced");
                match service.server.did_change(lsp_types::DidChangeTextDocumentParams {
                    text_document,
                    content_changes,
                }) {
                    Ok(_) => service.text_version = Some(version),
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
        let text = Rope::clone(&self.text);
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
        let text = Rope::clone(&self.text);
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
        let text = Rope::clone(&self.text);
        self.server
            .type_definition(to_proto::goto_definition(
                self.position_encoding(),
                &self.text,
                params,
            ))
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
        let text = Rope::clone(&self.text);
        self.server
            .implementation(to_proto::goto_definition(self.position_encoding(), &self.text, params))
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
        let text = Rope::clone(&self.text);
        self.server
            .references(lsp_types::ReferenceParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri: params.at.url },
                    position: to_proto::point(
                        self.position_encoding(),
                        &self.text,
                        params.at.point,
                    ),
                },
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
    use zi::{lstypes, Delta, Deltas, Point, PointRange, PositionEncoding, Text};

    pub fn goto_definition(
        encoding: PositionEncoding,
        text: &(impl Text + ?Sized),
        res: Option<lsp_types::GotoDefinitionResponse>,
    ) -> Option<lstypes::GotoDefinitionResponse> {
        let res = match res {
            None => lstypes::GotoDefinitionResponse::Array(vec![]),
            Some(lsp_types::GotoDefinitionResponse::Scalar(loc)) => {
                lstypes::GotoDefinitionResponse::Array(vec![location(encoding, text, loc)?])
            }
            Some(lsp_types::GotoDefinitionResponse::Array(locs)) => {
                lstypes::GotoDefinitionResponse::Array(
                    locs.into_iter().filter_map(|loc| location(encoding, text, loc)).collect(),
                )
            }
            Some(lsp_types::GotoDefinitionResponse::Link(links)) => {
                lstypes::GotoDefinitionResponse::Array(
                    links
                        .into_iter()
                        .filter_map(|link| {
                            Some(lstypes::Location {
                                uri: link.target_uri,
                                range: range(encoding, text, link.target_selection_range)?,
                            })
                        })
                        .collect(),
                )
            }
        };
        Some(res)
    }

    pub fn location(
        encoding: PositionEncoding,
        text: &(impl Text + ?Sized),
        loc: lsp_types::Location,
    ) -> Option<lstypes::Location> {
        Some(lstypes::Location { uri: loc.uri, range: range(encoding, text, loc.range)? })
    }

    pub fn deltas(
        encoding: PositionEncoding,
        text: &(impl Text + ?Sized),
        edits: impl IntoIterator<Item = lsp_types::TextEdit, IntoIter: ExactSizeIterator>,
    ) -> Option<Deltas<'static>> {
        let edits = edits.into_iter();
        let n = edits.len();
        let deltas = Deltas::new(edits.filter_map(|edit| {
            let range = text.point_range_to_byte_range(range(encoding, text, edit.range)?);
            Some(Delta::new(range, edit.new_text))
        }));

        // If any of the edits were invalid, return None.
        if deltas.len() < n { None } else { Some(deltas) }
    }

    pub fn range(
        encoding: PositionEncoding,
        text: &(impl Text + ?Sized),
        range: lsp_types::Range,
    ) -> Option<PointRange> {
        Some(PointRange::new(
            point(encoding, text, range.start)?,
            point(encoding, text, range.end)?,
        ))
    }

    pub fn point(
        encoding: PositionEncoding,
        text: &(impl Text + ?Sized),
        point: lsp_types::Position,
    ) -> Option<Point> {
        if point.line as usize > text.len_lines() {
            return None;
        }

        match encoding {
            PositionEncoding::Utf8 => {
                Some(Point::new(point.line as usize, point.character as usize))
            }
            PositionEncoding::Utf16 => {
                let line_start_byte = text.line_to_byte(point.line as usize);
                let line_start_cu = text.byte_to_utf16_cu(line_start_byte);
                if line_start_cu + point.character as usize > text.len_utf16_cu() {
                    return None;
                }

                let byte = text.utf16_cu_to_byte(line_start_cu + point.character as usize);
                Some(text.byte_to_point(byte))
            }
        }
    }
}

mod to_proto {
    use async_lsp::lsp_types;
    use zi::{lstypes, Deltas, Point, PointRange, PositionEncoding, Text};

    pub fn goto_definition(
        encoding: PositionEncoding,
        text: &(impl Text + ?Sized),
        params: lstypes::GotoDefinitionParams,
    ) -> lsp_types::GotoDefinitionParams {
        lsp_types::GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: params.at.url },
                position: point(encoding, text, params.at.point),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }
    }

    // For some reason, LSP defines change events that are distinct from `TextEdit`s.
    // The former is applied serially, while the latter is applied "atomically".
    // However, since our deltas are ordered and disjoint, we can just return them in order and because
    // they don't interfere we're all good.
    pub fn deltas(
        encoding: PositionEncoding,
        old_text: impl Text,
        deltas: &Deltas<'_>,
    ) -> Vec<lsp_types::TextDocumentContentChangeEvent> {
        deltas
            .iter()
            .map(|delta| lsp_types::TextDocumentContentChangeEvent {
                range: Some(byte_range(encoding, &old_text, delta.range())),
                text: delta.text().to_string(),
                range_length: None,
            })
            .collect()
    }

    pub fn byte_range(
        encoding: PositionEncoding,
        text: &(impl Text + ?Sized),
        range: std::ops::Range<usize>,
    ) -> lsp_types::Range {
        lsp_types::Range {
            start: byte(encoding, text, range.start),
            end: byte(encoding, text, range.end),
        }
    }

    pub fn byte(
        encoding: PositionEncoding,
        text: &(impl Text + ?Sized),
        byte: usize,
    ) -> lsp_types::Position {
        match encoding {
            PositionEncoding::Utf8 => point(encoding, text, text.byte_to_point(byte)),
            PositionEncoding::Utf16 => {
                let line = text.byte_to_line(byte);
                let line_start = text.byte_to_utf16_cu(text.line_to_byte(line));
                let col = text.byte_to_utf16_cu(byte) - line_start;
                lsp_types::Position::new(line as u32, col as u32)
            }
        }
    }

    pub fn range(
        encoding: PositionEncoding,
        text: &(impl Text + ?Sized),
        range: PointRange,
    ) -> lsp_types::Range {
        lsp_types::Range {
            start: point(encoding, text, range.start()),
            end: point(encoding, text, range.end()),
        }
    }

    pub fn point(
        encoding: PositionEncoding,
        text: &(impl Text + ?Sized),
        point: Point,
    ) -> lsp_types::Position {
        match encoding {
            PositionEncoding::Utf8 => {
                lsp_types::Position::new(point.line() as u32, point.col() as u32)
            }
            PositionEncoding::Utf16 => byte(encoding, text, text.point_to_byte(point)),
        }
    }
}
