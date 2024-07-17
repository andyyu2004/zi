use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::bail;
use futures_core::future::BoxFuture;
use futures_util::FutureExt;
use lsp_types::{self, OneOf, Url};
use zi_core::Point;

use super::{active_servers_of, callback, event, get, Result, Selector};
use crate::buffer::picker::{BufferPicker, BufferPickerEntry};
use crate::language_service::{lstypes, LanguageServiceInstance};
use crate::{
    lsp, BufferId, Editor, FileType, LanguageClient, LanguageService, LanguageServiceId, Location,
    OpenFlags, Resource, ViewId,
};

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
        Fut: Future<Output = Result<lstypes::GotoDefinitionResponse>> + 'static,
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
    ) -> impl Future<Output = Result<lstypes::GotoDefinitionResponse>> + 'static {
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
    ) -> impl Future<Output = Result<lstypes::GotoDefinitionResponse>> + 'static {
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
    ) -> impl Future<Output = Result<lstypes::GotoDefinitionResponse>> + 'static {
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
    ) -> impl Future<Output = Result<lstypes::GotoDefinitionResponse>> + 'static {
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
    ) -> impl Future<Output = Result<lstypes::GotoDefinitionResponse>> {
        let view = selector.select(self);
        self.find_definitions_(
            "textDocument/references",
            view,
            |cap| matches!(cap.references_provider, Some(OneOf::Left(true) | OneOf::Right(_))),
            |server, params| {
                server
                    .references(lstypes::ReferenceParams { at: params.at })
                    .map(|res| res.map(lstypes::GotoDefinitionResponse::Array))
            },
        )
    }

    fn find_definitions_<Fut>(
        &mut self,
        desc: &'static str,
        view: ViewId,
        has_cap: impl Fn(&lsp_types::ServerCapabilities) -> bool,
        f: impl FnOnce(&mut dyn LanguageService, lstypes::GotoDefinitionParams) -> Fut,
    ) -> impl Future<Output = Result<lstypes::GotoDefinitionResponse>> + 'static
    where
        Fut: Future<Output = Result<lstypes::GotoDefinitionResponse>> + 'static,
    {
        let res = active_servers_of!(self, view)
            .find(|server_id| has_cap(&self.active_language_services[server_id].capabilities()))
            .and_then(|server_id| {
                let (view, buf) = get!(self: view);
                let url = buf.url().clone();
                let server = self.active_language_services.get_mut(server_id).unwrap();
                let point = view.cursor();
                tracing::debug!(%url, %point, "language request definition");
                let fut = f(
                    &mut **server,
                    lstypes::GotoDefinitionParams {
                        at: lstypes::TextDocumentPointParams { url, point },
                    },
                );
                Some(fut)
            });

        async move {
            match res {
                None => bail!("no language server supports {desc}"),
                Some(fut) => {
                    let res = fut.await?;
                    tracing::debug!(?res, "lsp definition response");
                    Ok(res)
                }
            }
        }
    }

    fn lsp_root_path(&self, _server: LanguageServiceId) -> PathBuf {
        // TODO this should be configurable per language server
        std::env::current_dir().unwrap()
    }

    fn lsp_workspace_root(&self, server: LanguageServiceId) -> lsp_types::WorkspaceFolder {
        let uri = Url::from_file_path(self.lsp_root_path(server)).unwrap();
        lsp_types::WorkspaceFolder {
            name: uri
                .path_segments()
                .and_then(Iterator::last)
                .map_or("root".into(), |basename| basename.to_string()),
            uri,
        }
    }

    pub(super) fn spawn_language_services_for_ft(
        &mut self,
        buf: BufferId,
        ft: FileType,
    ) -> Result<()> {
        if let Some(config) = &self.language_config.languages.get(&ft) {
            for server_id in config.language_services.iter().cloned() {
                if self.active_language_services.contains_key(&server_id) {
                    // Language server already running
                    continue;
                }

                let client = LanguageClient::new(server_id, self.client());
                let root_path = self.lsp_root_path(server_id);
                let workspace_root = self.lsp_workspace_root(server_id);
                let (service, fut) =
                    self.language_config.language_services[&server_id].spawn(&root_path, client)?;
                let handle = tokio::spawn(fut);
                let mut service = LanguageServiceInstance::new(service, handle);

                callback(
                    &self.callbacks_tx,
                    "initializing language service",
                    async move {
                        service
                            .initialize(lstypes::InitializeParams {
                                process_id: std::process::id(),
                                capabilities: lsp::client_capabilities(),
                                workspace_folders: vec![workspace_root],
                            })
                            .await?;

                        Ok(service)
                    },
                    move |editor, mut service| {
                        service.initialized()?;
                        tracing::info!("language service initialized");

                        assert!(
                            editor.active_language_services.insert(server_id, service).is_none(),
                            "inserted duplicate language server"
                        );

                        editor
                            .active_language_services_by_ft
                            .entry(ft)
                            .or_default()
                            .push(server_id);

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
        res: lstypes::GotoDefinitionResponse,
    ) -> Result<impl Future<Output = Result<()>> + 'static> {
        let mut locations = match res {
            lstypes::GotoDefinitionResponse::Array(locations) => locations,
        };

        #[derive(Clone, Debug)]
        struct Entry {
            path: PathBuf,
            line: usize,
        }

        impl BufferPickerEntry for Entry {
            fn buffer_or_path(&self) -> Result<BufferId, &Path> {
                Err(&self.path)
            }

            fn point(&self) -> Option<Point> {
                Some(Point::new(self.line, 0).into())
            }
        }

        impl fmt::Display for Entry {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", self.path.display(), self.line)
            }
        }

        match &locations[..] {
            [] => bail!("no definition found"),
            [_] => Ok(Box::pin(self.lsp_jump_to_location(locations.pop().unwrap())?)
                as BoxFuture<'static, _>),
            _ => {
                self.open_static_picker::<BufferPicker<_>>(
                    Url::parse("view-group://lsp/picker").unwrap(),
                    "/",
                    (1, 1),
                    move |_, injector| {
                        for location in locations {
                            let Ok(path) = location.url.to_file_path() else { continue };
                            let entry =
                                Entry { path, line: location.range.start().line() as usize };
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
        location: lstypes::Location,
    ) -> Result<impl Future<Output = Result<()>> + 'static> {
        let path = location
            .url
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("lsp returned non-file uri: {}", location.url))?;

        let from = self.current_location();
        let open_fut =
            self.open(path, OpenFlags::SPAWN_LANGUAGE_SERVICES | OpenFlags::BACKGROUND)?;
        let client = self.client();
        Ok(async move {
            let buf = open_fut.await?;
            client
                .with(move |editor| editor.jump(from, Location::new(buf, location.range.start())))
                .await;
            Ok(())
        })
    }

    pub(crate) fn request_semantic_tokens(
        &mut self,
        selector: impl Selector<BufferId>,
    ) -> Option<impl Future<Output = Result<()>>> {
        let buf = selector.select(self);

        let buf_version = self.buffers[buf].version();
        tracing::info!(?buf, buf_version, "requesting semantic tokens");

        let client = self.client();

        let url = self.buffers[buf].url().clone();

        let Some((server, _caps)) = active_servers_of!(self, buf).find_map(|&server| {
            let caps = self.active_language_services[&server]
                .capabilities()
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

        let theme = self.theme();
        let s = self.active_language_services.get_mut(&server).unwrap();
        tracing::debug!(%url, "requesting semantic tokens full");
        let fut = s.semantic_tokens_full(theme, lstypes::SemanticTokensParams { url });

        Some(async move {
            let Some(marks) = fut.await? else { return Ok(()) };
            client
                .with(move |editor| {
                    let ns = editor.create_namespace("semantic-tokens");
                    editor[buf].replace_marks(ns, marks);
                })
                .await;

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
        tracing::info!(?buf, "requesting diagnostics");

        let (server_ids, futs) = active_servers_of!(self, buf)
            .filter_map(|&server_id| {
                let true = self.active_language_services[&server_id]
                    .capabilities()
                    .diagnostic_provider
                    .is_some()
                else {
                    return None;
                };
                let url = self.buffers[buf].url();
                let server = self.active_language_services.get_mut(&server_id).unwrap();
                let fut = server
                    .document_diagnostic(lstypes::DocumentDiagnosticParams { url: url.clone() });
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
                client.send(move |editor| {
                    editor.replace_diagnostics(path, None, res.diagnostics);
                    for (url, related) in res.related_documents {
                        let Ok(path) = url.to_file_path() else {
                            tracing::warn!(?url, "ignoring non-file related document diagnostics");
                            continue;
                        };
                        editor.replace_diagnostics(path, None, related)
                    }
                    Ok(())
                });
            }

            Ok(())
        }
    }
}
