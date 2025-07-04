use futures_util::TryFutureExt;

use super::*;
use crate::event::AsyncEventHandler;
use crate::{Editor, event, lstypes};

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
                            let url = editor[event.buf].file_url().cloned()?;
                            active_servers_of!(editor, event.buf).find_map(|server_id| {
                                let server =
                                    editor.active_language_services.get_mut(server_id).unwrap();
                                match server.formatting_capabilities() {
                                    Some(()) => {
                                        Some(server.format(lstypes::DocumentFormattingParams {
                                            url: url.clone(),
                                            options: lstypes::FormattingOptions { tab_size },
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
            editor.refresh_semantic_tokens(event.buf);
            event::HandlerResult::Continue
        })
    }

    pub(super) fn refresh_semantic_tokens(&mut self, buf: BufferId) {
        if let Some(fut) = self.request_semantic_tokens(buf) {
            self.spawn("semantic tokens", fut.map_err(Into::into))
        };
    }
}
