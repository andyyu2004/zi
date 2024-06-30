use futures_util::TryFutureExt;

use self::lsp::from_proto;
use super::*;
use crate::syntax::HighlightName;
use crate::Mark;

impl Editor {
    pub(super) fn format_before_save() -> impl AsyncEventHandler<Event = event::WillSaveBuffer> {
        event::async_handler::<event::WillSaveBuffer, _>(|client, event| async move {
            let (version, format_fut) = client
                .with(move |editor| {
                    let buffer = &editor[event.buf];
                    let buf_config = buffer.settings();
                    let tab_size = *buf_config.tab_width.read() as u32;
                    let format = *buf_config.format_on_save.read();

                    let buf_version = buffer.version();
                    let format_fut = format
                        .then(|| {
                            editor[event.buf].file_url().cloned().and_then(|uri| {
                                active_servers_of!(editor, event.buf).find_map(|server_id| {
                                    let server =
                                        editor.active_language_servers.get_mut(server_id).unwrap();
                                    match server.capabilities.document_formatting_provider {
                                        Some(
                                            lsp_types::OneOf::Left(true)
                                            | lsp_types::OneOf::Right(_),
                                        ) => Some((server.position_encoding(),server.formatting(
                                            lsp_types::DocumentFormattingParams {
                                                text_document: lsp_types::TextDocumentIdentifier {
                                                    uri: uri.clone(),
                                                },
                                                options: lsp_types::FormattingOptions {
                                                    tab_size,
                                                    insert_spaces: true,
                                                    trim_trailing_whitespace: Some(true),
                                                    insert_final_newline: Some(true),
                                                    trim_final_newlines: Some(true),
                                                    properties: Default::default(),
                                                },
                                                work_done_progress_params:
                                                    lsp_types::WorkDoneProgressParams::default(),
                                            },
                                        ))),
                                        _ => None,
                                    }
                                })
                            })
                        })
                        .flatten();

                    (buf_version, format_fut)
                })
                .await;

            if let Some((encoding, fut)) = format_fut {
                if let Some(edits) = fut.await? {
                    client
                        .with(move |editor| {
                            let buf = &editor[event.buf];
                            let text = buf.text();
                            let deltas = lsp::from_proto::deltas(encoding, text, edits);

                            if buf.version() == version {
                                match deltas {
                                    Some(deltas) => {
                                        editor.edit(event.buf, &deltas)?;
                                        editor[event.buf].snapshot(SnapshotFlags::empty());
                                    }
                                    None => {
                                        tracing::error!("ignoring invalid format edits")
                                    }
                                }
                            } else {
                                assert!(buf.version() > version, "version has gone down?");
                                tracing::info!(
                                    "buffer version changed, skipping formatting: {} > {version}",
                                    buf.version(),
                                );
                            }
                            Ok::<_, Error>(())
                        })
                        .await?;
                }
            }

            Ok(())
        })
    }

    pub(super) fn lsp_did_open(
        server_id: LanguageServerId,
    ) -> impl EventHandler<Event = event::DidOpenBuffer> {
        event::handler::<event::DidOpenBuffer>(move |editor, event| {
            let buf = &editor.buffers[event.buf];
            if let (Some(server), Some(uri)) =
                (editor.active_language_servers.get_mut(&server_id), buf.file_url())
            {
                tracing::debug!(?event, ?server_id, "lsp buffer did open");
                server
                    .did_open(lsp_types::DidOpenTextDocumentParams {
                        text_document: lsp_types::TextDocumentItem {
                            uri: uri.clone(),
                            language_id: buf.file_type().to_string(),
                            version: buf.version() as i32,
                            text: buf.text().to_string(),
                        },
                    })
                    .expect("lsp did_open failed");
            }
            event::HandlerResult::Continue
        })
    }

    pub(super) fn lsp_did_open_refresh_semantic_tokens()
    -> impl EventHandler<Event = event::DidOpenBuffer> {
        event::handler::<event::DidOpenBuffer>(move |editor, event| {
            editor.schedule_semantic_tokens(event.buf)
        })
    }

    pub(super) fn lsp_did_change_refresh_semantic_tokens()
    -> impl EventHandler<Event = event::DidChangeBuffer> {
        event::handler::<event::DidChangeBuffer>(move |editor, event| {
            editor.schedule_semantic_tokens(event.buf)
        })
    }

    fn schedule_semantic_tokens(&mut self, buf: BufferId) -> event::HandlerResult {
        fn semantic_tt_to_highlight(tt: &lsp_types::SemanticTokenType) -> Option<HighlightName> {
            use lsp_types::SemanticTokenType as Stt;
            Some(match tt {
                t if t == &Stt::NAMESPACE => HighlightName::NAMESPACE,
                t if t == &Stt::TYPE => HighlightName::TYPE,
                t if t == &Stt::STRUCT => HighlightName::TYPE,
                t if t == &Stt::CLASS => HighlightName::TYPE,
                t if t == &Stt::INTERFACE => HighlightName::TYPE,
                t if t == &Stt::ENUM => HighlightName::TYPE,
                t if t == &Stt::TYPE_PARAMETER => HighlightName::TYPE,
                t if t == &Stt::PARAMETER => HighlightName::PARAMETER,
                t if t == &Stt::VARIABLE => HighlightName::VARIABLE,
                t if t == &Stt::PROPERTY => HighlightName::PROPERTY,
                // t if t == &Stt::ENUM_MEMBER => HighlightName::ENUM_MEMBER,
                // t if t == &Stt::EVENT => HighlightName::EVENT,
                t if t == &Stt::FUNCTION => HighlightName::FUNCTION,
                t if t == &Stt::METHOD => HighlightName::FUNCTION,
                t if t == &Stt::MACRO => HighlightName::MACRO,
                t if t == &Stt::KEYWORD => HighlightName::KEYWORD,
                // t if t == &Stt::MODIFIER => HighlightName::MODIFIER,
                t if t == &Stt::COMMENT => HighlightName::COMMENT,
                t if t == &Stt::STRING => HighlightName::STRING,
                t if t == &Stt::NUMBER => HighlightName::NUMBER,
                t if t == &Stt::REGEXP => HighlightName::STRING,
                // t if t == &Stt::OPERATOR => HighlightName::OPERATOR,
                // t if t == &Stt::DECORATOR => HighlightName::DECORATOR,
                _ => return None,
            })
        }

        if let Some(fut) = self.request_semantic_tokens(buf) {
            self.callback("semantic tokens", fut.map_err(Into::into), move |editor, ()| {
                if editor.semantic_tokens.get(&buf).is_none() {
                    return Ok(());
                }

                let ns = editor.create_namespace("semantic-tokens");
                editor[buf].clear_marks(ns, ..);
                let Some(cache) = &editor.semantic_tokens.get(&buf) else { return Ok(()) };

                let Some(server) = editor.active_language_servers.get(&cache.server) else {
                    return Ok(());
                };

                let start = Instant::now();
                let encoding = server.position_encoding();

                let mut line = 0;
                let mut char = 0;
                let text = editor[buf].text();
                let marks = cache
                    .tokens
                    .iter()
                    .filter_map(|token| {
                        if token.delta_line > 0 {
                            char = 0;
                        }

                        line += token.delta_line;
                        char += token.delta_start;

                        let hl = semantic_tt_to_highlight(
                            &cache.legend.token_types[token.token_type as usize],
                        )
                        .map(|name| editor.highlight_id_by_name(name))?;

                        let point = from_proto::point(
                            encoding,
                            text,
                            lsp_types::Position::new(line, char),
                        )?;
                        let start = text.point_to_byte(point);
                        // TODO need to convert this length to the right encoding too...
                        Some(Mark::builder(ns, start).width(token.length as usize).hl(hl))
                    })
                    .collect::<Vec<_>>();

                for mark in marks {
                    editor[buf].create_mark(mark);
                }

                tracing::info!(time = ?start.elapsed(), "semantic tokens refreshed");

                Ok(())
            })
        };

        event::HandlerResult::Continue
    }

    pub(super) fn lsp_did_change_sync(
        server_id: LanguageServerId,
    ) -> impl EventHandler<Event = event::DidChangeBuffer> {
        // Sync event handler
        event::handler::<event::DidChangeBuffer>(move |editor, event| {
            tracing::trace!(buf = ?event.buf, "buffer did change");

            let buf = &editor.buffers[event.buf];
            if let (Some(server), Some(uri)) =
                (editor.active_language_servers.get_mut(&server_id), buf.file_url().cloned())
            {
                if !editor
                    .language_config
                    .languages
                    .get(&buf.file_type())
                    .map(|c| &c.language_servers)
                    .map_or(false, |servers| servers.contains(&server_id))
                {
                    return event::HandlerResult::Continue;
                }

                let encoding = server.position_encoding();

                let kind = match &server.capabilities.text_document_sync {
                    Some(cap) => match cap {
                        lsp_types::TextDocumentSyncCapability::Kind(kind) => kind,
                        lsp_types::TextDocumentSyncCapability::Options(opts) => {
                            match &opts.change {
                                Some(kind) => kind,
                                None => return event::HandlerResult::Continue,
                            }
                        }
                    },
                    None => return event::HandlerResult::Continue,
                };

                tracing::debug!(%uri, ?server_id, "lsp did_change");
                let text_document = lsp_types::VersionedTextDocumentIdentifier {
                    uri,
                    version: buf.version() as i32,
                };

                let content_changes = match *kind {
                    lsp_types::TextDocumentSyncKind::INCREMENTAL => {
                        lsp::to_proto::deltas(encoding, event.old_text.as_ref(), &event.deltas)
                    }
                    lsp_types::TextDocumentSyncKind::FULL => {
                        vec![lsp_types::TextDocumentContentChangeEvent {
                            range: None,
                            range_length: None,
                            text: buf.text().to_string(),
                        }]
                    }
                    lsp_types::TextDocumentSyncKind::NONE => return event::HandlerResult::Continue,
                    _ => unreachable!("invalid text document sync kind: {kind:?}"),
                };

                if let Err(err) = server.did_change(lsp_types::DidChangeTextDocumentParams {
                    text_document,
                    content_changes,
                }) {
                    tracing::error!(?err, "lsp did_change notification failed")
                }
            }
            event::HandlerResult::Continue
        })
    }
}
