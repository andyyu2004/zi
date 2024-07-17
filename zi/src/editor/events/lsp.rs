use futures_util::TryFutureExt;

use super::*;
use crate::event::AsyncEventHandler;
use crate::lsp::from_proto;
use crate::syntax::HighlightName;
use crate::{event, lstypes, Editor, Mark};

impl Editor {
    pub(crate) fn format_before_save() -> impl AsyncEventHandler<Event = event::WillSaveBuffer> {
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
                            let url = editor[event.buf].url().clone();
                            active_servers_of!(editor, event.buf).find_map(|server_id| {
                                let server =
                                    editor.active_language_services.get_mut(server_id).unwrap();
                                match server.capabilities().document_formatting_provider {
                                    Some(
                                        lsp_types::OneOf::Left(true) | lsp_types::OneOf::Right(_),
                                    ) => {
                                        Some(server.formatting(lstypes::DocumentFormattingParams {
                                            url: url.clone(),
                                            options: lsp_types::FormattingOptions {
                                                tab_size,
                                                insert_spaces: true,
                                                trim_trailing_whitespace: Some(true),
                                                insert_final_newline: Some(true),
                                                trim_final_newlines: Some(true),
                                                properties: Default::default(),
                                            },
                                        }))
                                    }
                                    _ => None,
                                }
                            })
                        })
                        .flatten();

                    (buf_version, format_fut)
                })
                .await;

            if let Some(fut) = format_fut {
                let deltas = fut.await?;
                client
                    .with(move |editor| {
                        let buf = &editor[event.buf];
                        if let Some(deltas) = deltas {
                            if buf.version() == version {
                                editor.edit(event.buf, &deltas)?;
                                editor[event.buf].snapshot(SnapshotFlags::empty());
                            } else {
                                assert!(buf.version() > version, "version has gone down?");
                                tracing::info!(
                                    "buffer version changed, skipping formatting: {} > {version}",
                                    buf.version(),
                                );
                            }
                        }

                        Ok::<_, Error>(())
                    })
                    .await?;
            }

            Ok(event::HandlerResult::Continue)
        })
    }

    pub(super) fn lsp_did_open_refresh_semantic_tokens()
    -> impl EventHandler<Self, Event = event::DidOpenBuffer> {
        zi_event::handler::<Editor, event::DidOpenBuffer>(move |editor, event| {
            editor.schedule_semantic_tokens(event.buf);
            event::HandlerResult::Continue
        })
    }

    pub(super) fn schedule_semantic_tokens(&mut self, buf: BufferId) {
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

                let Some(cache) = &editor.semantic_tokens.get(&buf) else { return Ok(()) };
                if cache.buf_version != editor[buf].version() {
                    return Ok(());
                }

                let ns = editor.create_namespace("semantic-tokens");

                let Some(cache) = &editor.semantic_tokens.get(&buf) else { return Ok(()) };
                let Some(server) = editor.active_language_services.get(&cache.server) else {
                    return Ok(());
                };

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
                        Some(Mark::builder(start).width(token.length as usize).hl(hl))
                    })
                    .collect::<Vec<_>>();

                let n = marks.len();
                let start = Instant::now();
                editor[buf].replace_marks(ns, marks);
                tracing::debug!(n, time = ?start.elapsed(), "created marks from semantic tokens");

                Ok(())
            })
        };
    }
}
