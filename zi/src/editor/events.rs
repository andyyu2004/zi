use super::*;

impl Editor {
    pub(super) fn will_save_buffer() -> impl AsyncEventHandler<Event = event::WillSaveBuffer> {
        event::async_handler::<event::WillSaveBuffer, _>(|client, event| async move {
            let (version, format_fut) = client
                .request(move |editor| {
                    let buffer = &editor[event.buf];
                    let buf_config = buffer.config();
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
                                        ) => Some(server.formatting(
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
                                        )),
                                        _ => None,
                                    }
                                })
                            })
                        })
                        .flatten();

                    (buf_version, format_fut)
                })
                .await;

            if let Some(fut) = format_fut {
                if let Some(edits) = fut.await? {
                    client
                        .request(move |editor| {
                            let buf = &editor[event.buf];
                            let text = buf.text();
                            let deltas = lsp::from_proto::deltas(text, edits);

                            if buf.version() == version {
                                editor.edit(event.buf, &deltas);
                                editor[event.buf].snapshot(SnapshotFlags::empty());
                            } else {
                                assert!(buf.version() > version, "version has gone down?");
                                tracing::info!(
                                    "buffer version changed, skipping formatting: {} > {version}",
                                    buf.version(),
                                );
                            }
                        })
                        .await;
                }
            }

            Ok(HandlerResult::Continue)
        })
    }

    pub(super) fn did_open_buffer(
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

    pub(super) fn did_change_buffer(
        server_id: LanguageServerId,
    ) -> impl EventHandler<Event = event::DidChangeBuffer> {
        event::handler::<event::DidChangeBuffer>(move |editor, event| {
            tracing::debug!(?event, "buffer did change");
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
                        lsp::to_proto::deltas_to_events(encoding, buf.text(), &event.deltas)
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
