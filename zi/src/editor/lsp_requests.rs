use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::bail;
use futures_core::future::BoxFuture;
use futures_util::FutureExt;
use zi_lsp::lsp_types::{self, OneOf, Url};
use zi_lsp::PositionEncoding;

use super::{active_servers_of, callback, event, get, Client, Result, Selector, SemanticTokens};
use crate::buffer::picker::{PathPicker, PathPickerEntry};
use crate::lsp::{self, from_proto, to_proto, LanguageClient, LanguageServer};
use crate::{BufferId, Editor, FileType, LanguageServerId, Location, OpenFlags, ViewId};

impl Editor {
    pub fn goto_definition(
        &mut self,
        selector: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<()>> {
        self.goto_definition_(selector, |editor, view| editor.find_definitions(view))
    }

    pub fn goto_declaration(
        &mut self,
        selector: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<()>> {
        self.goto_definition_(selector, |editor, view| editor.find_declarations(view))
    }

    pub fn goto_type_definition(
        &mut self,
        selector: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<()>> {
        self.goto_definition_(selector, |editor, view| editor.find_type_definitions(view))
    }

    pub fn goto_implementation(
        &mut self,
        selector: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<()>> {
        self.goto_definition_(selector, |editor, view| editor.find_implementations(view))
    }

    pub fn goto_references(
        &mut self,
        selector: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<()>> {
        self.goto_definition_(selector, |editor, view| editor.find_references(view))
    }

    fn goto_definition_<Fut>(
        &mut self,
        selector: impl Selector<ViewId>,
        f: impl FnOnce(&mut Self, ViewId) -> Fut,
    ) -> impl Future<Output = Result<()>>
    where
        Fut: Future<Output = Result<(PositionEncoding, lsp_types::GotoDefinitionResponse)>>
            + 'static,
    {
        let view = selector.select(self);
        let fut = f(self, view);
        let client = self.client();
        async move {
            let res = fut.await?;
            client.with(|editor| editor.lsp_jump_to_definitions(res)).await?.await?;
            Ok(())
        }
    }

    pub fn find_definitions(
        &mut self,
        selector: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<(PositionEncoding, lsp_types::GotoDefinitionResponse)>> + 'static
    {
        let view = selector.select(self);
        self.find_definitions_(
            "textDocument/definition",
            view,
            |cap| matches!(cap.definition_provider, Some(OneOf::Left(true) | OneOf::Right(_))),
            |server, params| server.definition(params),
        )
    }

    pub fn find_implementations(
        &mut self,
        selector: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<(PositionEncoding, lsp_types::GotoDefinitionResponse)>> + 'static
    {
        let view = selector.select(self);
        self.find_definitions_(
            "textDocument/implementation",
            view,
            |cap| {
                !matches!(
                    cap.implementation_provider,
                    None | Some(lsp_types::ImplementationProviderCapability::Simple(false))
                )
            },
            |server, params| server.implementation(params),
        )
    }

    pub fn find_declarations(
        &mut self,
        selector: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<(PositionEncoding, lsp_types::GotoDefinitionResponse)>> + 'static
    {
        let view = selector.select(self);
        self.find_definitions_(
            "textDocument/declaration",
            view,
            |cap| {
                !matches!(
                    cap.declaration_provider,
                    None | Some(lsp_types::DeclarationCapability::Simple(false))
                )
            },
            |server, params| server.definition(params),
        )
    }

    pub fn find_type_definitions(
        &mut self,
        selector: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<(PositionEncoding, lsp_types::GotoDefinitionResponse)>> + 'static
    {
        let view = selector.select(self);
        self.find_definitions_(
            "textDocument/typeDefinition",
            view,
            |cap| {
                !matches!(
                    cap.type_definition_provider,
                    None | Some(lsp_types::TypeDefinitionProviderCapability::Simple(false))
                )
            },
            |server, params| server.type_definition(params),
        )
    }

    pub fn find_references(
        &mut self,
        selector: impl Selector<ViewId>,
    ) -> impl Future<Output = Result<(PositionEncoding, lsp_types::GotoDefinitionResponse)>> {
        let view = selector.select(self);
        self.find_definitions_(
            "textDocument/references",
            view,
            |cap| matches!(cap.references_provider, Some(OneOf::Left(true) | OneOf::Right(_))),
            |server, params| {
                server
                    .references(lsp_types::ReferenceParams {
                        text_document_position: params.text_document_position_params,
                        context: lsp_types::ReferenceContext { include_declaration: true },
                        partial_result_params: Default::default(),
                        work_done_progress_params: Default::default(),
                    })
                    .map(|res| res.map(|opt| opt.map(lsp_types::GotoDefinitionResponse::Array)))
            },
        )
    }

    fn find_definitions_<Fut>(
        &mut self,
        desc: &'static str,
        view: ViewId,
        has_cap: impl Fn(&lsp_types::ServerCapabilities) -> bool,
        f: impl FnOnce(&mut LanguageServer, lsp_types::GotoDefinitionParams) -> Fut,
    ) -> impl Future<Output = Result<(PositionEncoding, lsp_types::GotoDefinitionResponse)>> + 'static
    where
        Fut: Future<Output = zi_lsp::Result<Option<lsp_types::GotoDefinitionResponse>>> + 'static,
    {
        let res = active_servers_of!(self, view)
            .find(|server_id| has_cap(&self.active_language_servers[server_id].capabilities))
            .and_then(|server_id| {
                let (view, buf) = get!(self: view);
                let uri = buf.file_url()?;
                let server = self.active_language_servers.get_mut(server_id).unwrap();
                let point = view.cursor();
                let encoding = server.position_encoding();
                tracing::debug!(%uri, %point, "lsp request definition");
                let fut = f(
                    server,
                    lsp_types::GotoDefinitionParams {
                        text_document_position_params: lsp_types::TextDocumentPositionParams {
                            text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                            position: to_proto::point(encoding, &buf.text(), point),
                        },
                        work_done_progress_params: lsp_types::WorkDoneProgressParams {
                            work_done_token: None,
                        },
                        partial_result_params: lsp_types::PartialResultParams {
                            partial_result_token: None,
                        },
                    },
                );
                Some((encoding, fut))
            });

        async move {
            match res {
                None => bail!("no language server supports {desc}"),
                Some((encoding, fut)) => {
                    let res = fut.await?;
                    tracing::debug!(?res, "lsp definition response");
                    let res = match res {
                        None => lsp_types::GotoDefinitionResponse::Array(Default::default()),
                        Some(res) => res,
                    };
                    Ok((encoding, res))
                }
            }
        }
    }

    fn lsp_root_path(&self, _server: LanguageServerId) -> PathBuf {
        // TODO this should be configurable per language server
        std::env::current_dir().unwrap()
    }

    fn lsp_workspace_root(&self, server: LanguageServerId) -> lsp_types::WorkspaceFolder {
        let uri = Url::from_file_path(self.lsp_root_path(server)).unwrap();
        lsp_types::WorkspaceFolder {
            name: uri
                .path_segments()
                .and_then(Iterator::last)
                .map_or("root".into(), |basename| basename.to_string()),
            uri,
        }
    }

    pub(super) fn spawn_language_servers_for_ft(
        &mut self,
        buf: BufferId,
        ft: FileType,
    ) -> zi_lsp::Result<()> {
        if let Some(config) = &self.language_config.languages.get(&ft) {
            for server_id in config.language_servers.iter().cloned() {
                if self.active_language_servers.contains_key(&server_id) {
                    // Language server already running
                    continue;
                }

                let client = LanguageClient::new(server_id, self.client());
                let root_path = self.lsp_root_path(server_id);
                let workspace_root = self.lsp_workspace_root(server_id);
                let (mut server, fut) =
                    self.language_config.language_servers[&server_id].spawn(&root_path, client)?;
                let handle = tokio::spawn(fut);

                callback(
                    &self.callbacks_tx,
                    "initializing language server",
                    async move {
                        let res = server
                            .initialize(lsp_types::InitializeParams {
                                process_id: Some(std::process::id()),
                                capabilities: lsp::client_capabilities(),
                                workspace_folders: Some(vec![workspace_root]),
                                ..Default::default()
                            })
                            .await?;

                        Ok((res, server))
                    },
                    move |editor, (res, mut server)| {
                        let span = tracing::info_span!("lsp initialized", %server_id);
                        let _guard = span.enter();
                        server.initialized(lsp_types::InitializedParams {})?;

                        let server = LanguageServer::new(res.capabilities, handle, server);
                        tracing::info!(encoding = ?server.position_encoding(), "lsp initialized");

                        assert!(
                            editor.active_language_servers.insert(server_id, server).is_none(),
                            "inserted duplicate language server"
                        );

                        editor
                            .active_language_servers_for_ft
                            .entry(ft)
                            .or_default()
                            .push(server_id);

                        subscribe_per_server_lsp_event_handlers(server_id);

                        // Must dispatch this event after the server is inserted
                        // FIXME this is wrong to just generate an event and send it to all
                        // language servers.
                        // First if there are multiple iterations of this loop they will receive
                        // the event more than once.
                        // Second, not all languages have the same capabilities.
                        editor.dispatch(event::DidOpenBuffer { buf });
                        Ok(())
                    },
                );
            }
        }

        Ok(())
    }

    fn lsp_jump_to_definitions(
        &mut self,
        (encoding, res): (PositionEncoding, lsp_types::GotoDefinitionResponse),
    ) -> Result<impl Future<Output = Result<()>> + 'static> {
        let mut locations = match res {
            lsp_types::GotoDefinitionResponse::Scalar(location) => vec![location],
            lsp_types::GotoDefinitionResponse::Array(locations) => locations,
            lsp_types::GotoDefinitionResponse::Link(links) => links
                .into_iter()
                .map(|link| lsp_types::Location {
                    uri: link.target_uri,
                    range: link.target_selection_range,
                })
                .collect(),
        };

        #[derive(Clone, Debug)]
        struct Entry {
            path: PathBuf,
            line: usize,
        }

        impl PathPickerEntry for Entry {
            fn path(&self) -> &Path {
                &self.path
            }

            fn line(&self) -> Option<usize> {
                Some(self.line)
            }
        }

        impl fmt::Display for Entry {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", self.path.display(), self.line)
            }
        }

        match &locations[..] {
            [] => bail!("no definition found"),
            [_] => Ok(Box::pin(self.lsp_jump_to_location(encoding, locations.pop().unwrap())?)
                as BoxFuture<'static, _>),
            _ => {
                self.open_static_picker::<PathPicker<_>>(
                    Url::parse("view-group://lsp/picker").unwrap(),
                    "/",
                    (1, 1),
                    move |_, injector| {
                        for location in locations {
                            let Ok(path) = location.uri.to_file_path() else { continue };
                            let entry = Entry { path, line: location.range.start.line as usize };
                            if injector.push(entry).is_err() {
                                break;
                            }
                        }
                    },
                );
                Ok(Box::pin(async { Ok(()) }))
            }
        }
    }

    fn lsp_jump_to_location(
        &mut self,
        encoding: PositionEncoding,
        location: lsp_types::Location,
    ) -> Result<impl Future<Output = Result<()>> + 'static> {
        let path = location
            .uri
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("lsp returned non-file uri: {}", location.uri))?;

        let from = self.current_location();
        let open_fut =
            self.open(path, OpenFlags::SPAWN_LANGUAGE_SERVERS | OpenFlags::BACKGROUND)?;
        let client = self.client();
        Ok(async move {
            let buf = open_fut.await?;
            client
                .with(move |editor| {
                    let text = editor[buf].text();
                    let point = from_proto::point(encoding, text, location.range.start);
                    editor.jump(from, Location::new(buf, point));
                })
                .await;
            Ok(())
        })
    }

    pub(crate) fn request_semantic_tokens(
        &mut self,
        selector: impl Selector<BufferId>,
    ) -> Option<impl Future<Output = zi_lsp::Result<()>>> {
        let buf = selector.select(self);
        let client = self.client();

        let Some(uri) = self.buffers[buf].file_url().cloned() else {
            tracing::warn!(?buf, "cannot request semantic tokens for non-file buffer");
            return None;
        };

        let Some((server, caps)) = active_servers_of!(self, buf).find_map(|&server| {
            let caps = self.active_language_servers[&server]
                .capabilities
                .semantic_tokens_provider
                .clone()?;

            let caps = match caps {
                lsp_types::SemanticTokensServerCapabilities::SemanticTokensOptions(opts) => opts,
                lsp_types::SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                    opts,
                ) => opts.semantic_tokens_options,
            };

            // Don't care if the server supports only range, we don't use it.
            caps.full.as_ref()?;

            Some((server, caps))
        }) else {
            tracing::warn!(?buf, "no active language server for buffer supports semantic tokens");
            return None;
        };

        let tokens = match self.semantic_tokens.entry(buf) {
            Entry::Occupied(mut entry) => {
                if entry.get().server == server {
                    entry.into_mut()
                } else {
                    // If the server is different (e.g. the prior one died), we need to clear the tokens.
                    entry.insert(SemanticTokens {
                        server,
                        legend: caps.legend.clone(),
                        tokens: Default::default(),
                        last_request_id: None,
                    });
                    entry.into_mut()
                }
            }
            Entry::Vacant(entry) => entry.insert(SemanticTokens {
                server,
                legend: caps.legend.clone(),
                tokens: Default::default(),
                last_request_id: None,
            }),
        };

        enum Res {
            Full(BoxFuture<'static, zi_lsp::Result<Option<lsp_types::SemanticTokensResult>>>),
            Delta(
                BoxFuture<
                    'static,
                    zi_lsp::Result<Option<lsp_types::SemanticTokensFullDeltaResult>>,
                >,
            ),
        }

        let s = self.active_language_servers.get_mut(&server).unwrap();
        let res = match (caps.full, tokens.last_request_id.clone()) {
            (
                Some(lsp_types::SemanticTokensFullOptions::Delta { delta: Some(true) }),
                Some(previous_result_id),
                // `if false` here to avoid taking this branch as it's incomplete since the editing is not implemented
            ) if false => {
                Res::Delta(s.semantic_tokens_full_delta(lsp_types::SemanticTokensDeltaParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                    previous_result_id,
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                }))
            }
            _ => Res::Full(s.semantic_tokens_full(lsp_types::SemanticTokensParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })),
        };

        Some(async move {
            match res {
                Res::Delta(fut) => {
                    tracing::debug!(%uri, "requesting semantic tokens delta");
                    let res = fut.await?;
                    client
                        .with(move |editor| {
                            let cache = &mut editor.semantic_tokens.get_mut(&buf).unwrap();
                            match res {
                                Some(res) => match res {
                                    lsp_types::SemanticTokensFullDeltaResult::Tokens(tokens) => {
                                        cache.last_request_id = tokens.result_id;
                                        cache.tokens = tokens.data;
                                    }
                                    lsp_types::SemanticTokensFullDeltaResult::TokensDelta(delta) => {
                                        cache.last_request_id = delta.result_id;
                                        for edit in delta.edits {
                                            // It's still entirely how multiple edits compose, are the "sequential" or "simultaneous"?
                                            // https://github.com/microsoft/vscode-extension-samples/blob/5ae1f7787122812dcc84e37427ca90af5ee09f14/semantic-tokens-sample/vscode.proposed.d.ts#L131
                                            todo!("{edit:?}");
                                        }
                                    }
                                    lsp_types::SemanticTokensFullDeltaResult::PartialTokensDelta {
                                        ..
                                    } => unreachable!("did not ask for partial results"),
                                },
                                None => {
                                    cache.last_request_id = None;
                                    cache.tokens.clear();
                                }
                            }
                        })
                        .await;
                }
                Res::Full(fut) => {
                    tracing::debug!(%uri, "requesting semantic tokens full");

                    let res = fut.await?;
                    client
                        .with(move |editor| {
                            let cache = &mut editor.semantic_tokens.get_mut(&buf).unwrap();
                            match res {
                                Some(res) => match res {
                                    lsp_types::SemanticTokensResult::Tokens(tokens) => {
                                        cache.last_request_id = tokens.result_id;
                                        cache.tokens = tokens.data;
                                    }
                                    lsp_types::SemanticTokensResult::Partial(_) => {
                                        unreachable!("did not ask for partial results")
                                    }
                                },
                                None => {
                                    cache.last_request_id = None;
                                    cache.tokens.clear();
                                }
                            }
                        })
                        .await;
                }
            }

            Ok(())
        })
    }

    /// Pull diagnostics using the `textDocument/diagnostic` request.
    /// These can be accessed via [`Editor::lsp_diagnostics`].
    pub fn request_diagnostics(
        &mut self,
        selector: impl Selector<BufferId>,
    ) -> impl Future<Output = Result<()>> {
        let buf = selector.select(self);
        tracing::info!("requesting diagnostics for buffer {buf:?}");

        async fn update_related_docs(
            client: &Client,
            server_id: LanguageServerId,
            related_documents: Option<HashMap<Url, lsp_types::DocumentDiagnosticReportKind>>,
        ) {
            for (url, related) in related_documents.into_iter().flatten() {
                let Ok(path) = url.to_file_path() else {
                    tracing::warn!(?url, "ignoring non-file related document diagnostics");
                    continue;
                };
                match related {
                    lsp_types::DocumentDiagnosticReportKind::Full(report) => {
                        client
                            .with(move |editor| {
                                editor.update_diagnostics(server_id, path, None, report.items)
                            })
                            .await;
                    }
                    lsp_types::DocumentDiagnosticReportKind::Unchanged(_) => {}
                }
            }
        }

        let (server_ids, futs) = active_servers_of!(self, buf)
            .filter_map(|&server_id| {
                let true = self.active_language_servers[&server_id]
                    .capabilities
                    .diagnostic_provider
                    .is_some()
                else {
                    return None;
                };
                let uri = self.buffers[buf].file_url()?.clone();
                let server = self.active_language_servers.get_mut(&server_id).unwrap();
                let fut = server.document_diagnostic(lsp_types::DocumentDiagnosticParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri },
                    identifier: None,
                    previous_result_id: None,
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                });
                Some((server_id, fut))
            })
            .unzip::<_, _, Vec<_>, Vec<_>>();

        let path = self[buf].path();
        let client = self.client();
        async move {
            let Some(path) = path else { return Ok(()) };

            if server_ids.is_empty() {
                tracing::info!(
                    ?path,
                    "no active language server for buffer supports pull diagnostics"
                );

                return Ok(());
            }

            let responses = futures_util::future::try_join_all(futs).await?;
            for (server_id, res) in server_ids.into_iter().zip(responses) {
                tracing::debug!(?server_id, ?path, ?res, "diagnostic request response");

                let path = path.clone();
                match res {
                    lsp_types::DocumentDiagnosticReportResult::Report(report) => match report {
                        lsp_types::DocumentDiagnosticReport::Full(report) => {
                            client
                                .with(move |editor| {
                                    editor.update_diagnostics(
                                        server_id,
                                        path,
                                        None,
                                        report.full_document_diagnostic_report.items,
                                    )
                                })
                                .await;

                            update_related_docs(&client, server_id, report.related_documents).await;
                        }
                        lsp_types::DocumentDiagnosticReport::Unchanged(_) => {}
                    },
                    lsp_types::DocumentDiagnosticReportResult::Partial(report) => {
                        update_related_docs(&client, server_id, report.related_documents).await;
                    }
                }
            }

            Ok(())
        }
    }
}

fn subscribe_per_server_lsp_event_handlers(server_id: LanguageServerId) {
    // TODO check capabilities
    event::subscribe::<event::DidChangeBuffer>(Editor::lsp_did_change_sync(server_id));
    event::subscribe::<event::DidOpenBuffer>(Editor::lsp_did_open(server_id));
}
