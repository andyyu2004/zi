pub(crate) mod cursor;

use std::fs::File;
use std::future::Future;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;
use ropey::{Rope, RopeBuilder, RopeSlice};
use rustc_hash::FxHashMap;
use slotmap::SlotMap;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use zi_lsp::{lsp_types, LanguageServer as _};

use crate::event::{self, Event, KeyEvent};
use crate::keymap::{Action, Keymap};
use crate::lsp::{self, LanguageClient, LanguageServer};
use crate::motion::Motion;
use crate::syntax::Theme;
use crate::view::HasViewId;
use crate::{
    language, Buffer, BufferId, Direction, Error, LanguageId, LanguageServerId, Mode, View, ViewId,
};

pub struct Editor {
    mode: Mode,
    keymap: Keymap,
    buffers: SlotMap<BufferId, Buffer>,
    views: SlotMap<ViewId, View>,
    active_view: ViewId,
    theme: Theme,
    language_servers: FxHashMap<LanguageServerId, LanguageServer>,
    tx: CallbacksSender,
    language_config: language::Config,
}

/// Get the active view and buffer.
/// This needs to be a macro so rust can figure out the mutable borrows are disjoint
macro_rules! active {
    ($editor:ident) => {
        active!($editor: $editor.active_view)
    };
    ($editor:ident: $view:expr) => {{
        #[allow(unused_imports)]
        use $crate::view::HasViewId as _;
        let view = &mut $editor.views[$view.view_id()];
        let buf = &mut $editor.buffers[view.buffer()];
        (view, buf)
    }};
}

pub(crate) use active;

use self::cursor::SetCursorFlags;

pub type EditorCallback = Box<dyn FnOnce(&mut Editor) -> Result<(), Error>>;

pub type Callbacks = impl Stream<Item = CallbackFuture>;

type CallbackFuture = Pin<Box<dyn Future<Output = Result<EditorCallback, Error>> + Send>>;

type CallbacksSender = UnboundedSender<CallbackFuture>;

// Adaptor for tokio's channel to be a futures Stream
struct ChannelStream<T>(UnboundedReceiver<T>);

impl<T> Stream for ChannelStream<T> {
    type Item = T;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.poll_recv(cx)
    }
}

impl Editor {
    /// Create a new editor with a scratch buffer.
    /// Returns the editor instance and a stream of callbacks.
    /// The callback stream must be polled and the resulting callback executed on the editor.
    pub fn new() -> (Self, Callbacks) {
        let theme = Theme::default();
        let mut buffers = SlotMap::default();
        let buf =
            buffers.insert_with_key(|id| Buffer::new(id, LanguageId::TEXT, "scratch", "", &theme));
        let mut views = SlotMap::default();
        let active_view = views.insert_with_key(|id| View::new(id, buf));

        // Using an unbounded channel as we need `tx.send()` to be sync.
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let editor = Self {
            buffers,
            views,
            active_view,
            tx,
            language_config: Default::default(),
            language_servers: Default::default(),
            mode: Default::default(),
            keymap: Default::default(),
            theme: Default::default(),
        };
        (editor, ChannelStream(rx))
    }

    pub fn open(&mut self, path: impl AsRef<Path>) -> Result<BufferId, zi_lsp::Error> {
        let path = path.as_ref();

        for buf in self.buffers.values() {
            if buf.path() == path {
                self.views[self.active_view].set_buffer(buf.id());
                return Ok(buf.id());
            }
        }

        let rope = if path.exists() {
            let reader = BufReader::new(File::open(path)?);
            let mut builder = RopeBuilder::new();
            for line in reader.lines() {
                builder.append(line?.as_str());
                builder.append("\n");
            }
            builder.finish()
        } else {
            Rope::new()
        };

        let lang = LanguageId::detect(path);
        tracing::debug!(%lang, ?path, "detected language");
        let buf = self
            .buffers
            .insert_with_key(|id| Buffer::new(id, lang.clone(), path, rope, &self.theme));
        self.views[self.active_view].set_buffer(buf);

        if let Some(config) = &self.language_config.languages.get(&lang) {
            for server_id in config.language_servers.iter().cloned() {
                let server_config = &self.language_config.language_servers[&server_id];
                let command = &server_config.command;
                let args = &server_config.args;
                tracing::debug!(%server_id, ?command, ?args, "initializing language server");
                let mut server = zi_lsp::Server::start(LanguageClient, ".", command, &args[..])?;
                callback(
                    &self.tx,
                    async move {
                        let res = server
                            .initialize(lsp_types::InitializeParams {
                                capabilities: lsp::capabilities(),
                                ..Default::default()
                            })
                            .await?;
                        tracing::debug!("lsp initialized");
                        server.initialized(lsp_types::InitializedParams {})?;

                        Ok((server, res, buf))
                    },
                    |editor, (server, res, buf)| {
                        // TODO check capabilities
                        event::register(event::handler::<event::DidChangeBuffer>(
                            |editor, event| {
                                tracing::debug!(?event, "buffer did change");
                                let buf = &editor.buffers[event.buffer_id];
                                if let Some(uri) = buf.url() {
                                    // TODO only send to relevant language servers
                                    for (server_id, server) in &mut editor.language_servers {
                                        tracing::debug!(%uri, ?server_id, "lsp did_change");
                                        server
                                            .did_change(lsp_types::DidChangeTextDocumentParams {
                                                text_document:
                                                    lsp_types::VersionedTextDocumentIdentifier {
                                                        uri: uri.clone(),
                                                        version: buf.version() as i32,
                                                    },
                                                content_changes: vec![
                                                    lsp_types::TextDocumentContentChangeEvent {
                                                        range: None,
                                                        range_length: None,
                                                        text: buf.text().to_string(),
                                                    },
                                                ],
                                            })
                                            .expect("lsp did_change failed");
                                    }
                                }
                            },
                        ));

                        event::register(event::handler::<event::DidOpenBuffer>(|editor, event| {
                            let buf = &editor.buffers[event.buffer_id];
                            if let Some(uri) = buf.url() {
                                for (server_id, server) in &mut editor.language_servers {
                                    tracing::debug!(?event, ?server_id, "lsp buffer did open");
                                    server
                                        .did_open(lsp_types::DidOpenTextDocumentParams {
                                            text_document: lsp_types::TextDocumentItem {
                                                uri: uri.clone(),
                                                language_id: buf.language_id().to_string(),
                                                version: buf.version() as i32,
                                                text: buf.text().to_string(),
                                            },
                                        })
                                        .expect("lsp did_open failed");
                                }
                            }
                        }));

                        editor.language_servers.insert(
                            server_id,
                            LanguageServer { server, capabilities: res.capabilities },
                        );

                        editor.dispatch(event::DidOpenBuffer { buffer_id: buf });
                        Ok(())
                    },
                );
            }
        }

        Ok(buf)
    }

    pub async fn cleanup(&mut self) {
        for server in std::mem::take(&mut self.language_servers).into_values() {
            // TODO shutdown concurrently
            let _ = server.shutdown().await;
        }
    }

    pub fn should_quit(&self) -> bool {
        self.views.is_empty()
    }

    #[inline]
    pub fn on_key(&mut self, key: KeyEvent) {
        if let Some(f) = self.keymap.on_key(self.mode, key) {
            match f {
                Action::Fn(f) => f(self),
                Action::Insert(c) => self.insert_char(c),
            }
        }
    }

    #[inline]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    #[inline]
    pub fn set_mode(&mut self, mode: Mode) {
        if let (Mode::Insert, Mode::Normal) = (self.mode, mode) {
            let (view, buf) = active!(self);
            view.move_cursor(mode, buf, Direction::Left);
        }

        self.mode = mode;
    }

    #[inline]
    pub fn active_view(&self) -> &View {
        self.view(self.active_view)
    }

    #[inline]
    pub fn view(&self, id: ViewId) -> &View {
        self.views.get(id).expect("got bad view id?")
    }

    #[inline]
    pub fn buffer(&self, id: BufferId) -> &Buffer {
        self.buffers.get(id).expect("got bad buffer id?")
    }

    #[inline]
    pub fn active(&self) -> (&View, &Buffer) {
        let view = self.active_view();
        let buffer = self.buffer(view.buffer());
        (view, buffer)
    }

    pub fn insert_char(&mut self, c: char) {
        // Don't care if we're actually in insert mode, that's more a key binding namespace.
        let (view, buf) = active!(self);
        let cursor = view.cursor();
        buf.insert_char(cursor, c);
        match c {
            '\n' => {
                view.move_cursor(self.mode, buf, Direction::Down);
                view.move_cursor(self.mode, buf, Direction::Left);
            }
            _ => view.move_cursor(self.mode, buf, Direction::Right),
        }

        let event = event::DidChangeBuffer { buffer_id: buf.id() };
        self.dispatch(event);
    }

    fn dispatch(&mut self, event: impl Event) {
        event::dispatch(self, event);
    }

    pub fn insert(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    pub fn current_line(&self) -> RopeSlice<'_> {
        let (view, buffer) = self.active();
        let cursor = view.cursor();
        let text = buffer.text();
        text.line(cursor.line().idx())
    }

    pub fn current_char(&self) -> char {
        let (view, buffer) = self.active();
        let cursor = view.cursor();
        let text = buffer.text();
        text.line(cursor.line().idx()).char(cursor.col().idx())
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn motion(&mut self, motion: impl Motion) {
        let (view, buf) = active!(self);
        let pos = motion.motion(buf.text().slice(..), view.cursor());
        view.set_cursor(self.mode, buf, pos, SetCursorFlags::empty());
    }

    pub(crate) fn go_to_definition(&mut self) {
        for server in self.language_servers.values_mut() {
            match &server.capabilities.definition_provider {
                Some(lsp_types::OneOf::Left(true) | lsp_types::OneOf::Right(_)) => (),
                _ => continue,
            }

            let (view, buf) = active!(self);
            let pos = view.cursor();

            if let Some(uri) = buf.url() {
                tracing::debug!(%uri, %pos, "lsp request definition");
                let fut = server.definition(lsp_types::GotoDefinitionParams {
                    text_document_position_params: lsp_types::TextDocumentPositionParams {
                        text_document: lsp_types::TextDocumentIdentifier { uri },
                        position: pos.into(),
                    },
                    work_done_progress_params: lsp_types::WorkDoneProgressParams {
                        work_done_token: None,
                    },
                    partial_result_params: lsp_types::PartialResultParams {
                        partial_result_token: None,
                    },
                });

                self.callback(async move { Ok(fut.await?) }, |editor, res| {
                    tracing::debug!(?res, "lsp definition response");
                    editor.jump_to_definition(res)?;
                    Ok(())
                });

                // Send the request to the first server that supports it
                break;
            }
        }
    }

    pub fn close_active_view(&mut self) {
        self.close_view(self.active_view);
        // TODO don't jump to some random view
        self.active_view = self.views.keys().next().unwrap_or_default();
    }

    pub fn close_view(&mut self, id: impl HasViewId) {
        let id = id.view_id();
        assert!(self.views.remove(id).is_some(), "tried to close non-existent view");
    }

    fn jump_to_definition(
        &mut self,
        res: Option<lsp_types::GotoDefinitionResponse>,
    ) -> Result<(), Error> {
        let locations = match res {
            None => vec![],
            Some(lsp_types::GotoDefinitionResponse::Scalar(location)) => vec![location],
            Some(lsp_types::GotoDefinitionResponse::Array(locations)) => locations,
            Some(lsp_types::GotoDefinitionResponse::Link(links)) => links
                .into_iter()
                .map(|link| lsp_types::Location { uri: link.target_uri, range: link.target_range })
                .collect(),
        };

        match &locations[..] {
            [] => (),
            [location] => self.jump_to_location(location)?,
            _ => {
                tracing::warn!("multiple definitions not supported yet");
            }
        };

        Ok(())
    }

    fn jump_to_location(&mut self, location: &lsp_types::Location) -> Result<(), Error> {
        let path = location
            .uri
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("lsp returned non-file uri: {}", location.uri))?;

        let buf_id = self.open(path)?;
        let (_view, buf) = active!(self);
        assert_eq!(buf.id(), buf_id, "opened buffer should have been set as active");
        self.set_active_cursor(location.range.start);

        Ok(())
    }

    fn callback<R: 'static>(
        &mut self,
        fut: impl Future<Output = Result<R, Error>> + Send + 'static,
        f: impl FnOnce(&mut Editor, R) -> Result<(), Error> + Send + 'static,
    ) {
        callback(&self.tx, fut, f);
    }
}

fn callback<R: 'static>(
    tx: &CallbacksSender,
    fut: impl Future<Output = Result<R, Error>> + Send + 'static,
    f: impl FnOnce(&mut Editor, R) -> Result<(), Error> + Send + 'static,
) {
    tx.send(Box::pin(async move {
        let res = fut.await?;
        Ok(Box::new(move |editor: &mut Editor| f(editor, res))
            as Box<dyn FnOnce(&mut Editor) -> Result<(), Error>>)
    }))
    .expect("send failed");
}
