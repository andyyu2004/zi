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

use crate::event::KeyEvent;
use crate::keymap::{Action, Keymap};
use crate::lsp::LanguageClient;
use crate::motion::Motion;
use crate::syntax::Theme;
use crate::{Buffer, BufferId, Direction, LanguageServerId, Mode, View, ViewId};

pub struct Editor {
    pub quit: bool, // tmp hack
    mode: Mode,
    keymap: Keymap,
    buffers: SlotMap<BufferId, Buffer>,
    views: SlotMap<ViewId, View>,
    active_view: ViewId,
    theme: Theme,
    language_servers: FxHashMap<LanguageServerId, zi_lsp::Server>,
    tx: UnboundedSender<Pin<Box<dyn Future<Output = EditorCallback> + Send>>>,
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

pub type EditorCallback = Box<dyn FnOnce(&mut Editor)>;

pub type Callbacks = impl Stream<Item = Pin<Box<dyn Future<Output = EditorCallback> + Send>>>;

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
        let buf = buffers.insert_with_key(|id| Buffer::new(id, "scratch", "", &theme));
        let mut views = SlotMap::default();
        let active_view = views.insert_with_key(|id| View::new(id, buf));

        // Using an unbounded channel as we need `send` to be sync.

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let editor = Self {
            buffers,
            views,
            active_view,
            quit: false,
            tx,
            language_servers: Default::default(),
            mode: Default::default(),
            keymap: Default::default(),
            theme: Default::default(),
        };
        (editor, ChannelStream(rx))
    }

    pub fn open(&mut self, path: impl AsRef<Path>) -> Result<(), zi_lsp::Error> {
        let path = path.as_ref();
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

        if path.extension() == Some("rs".as_ref()) {
            let id = LanguageServerId::RUST_ANALYZER;
            let server = zi_lsp::Server::start(LanguageClient, ".", "rust-analyzer")?;
            self.language_servers.insert(id, server);
        }

        let buf = self.buffers.insert_with_key(|id| Buffer::new(id, path, rope, &self.theme));
        self.active_view = self.views.insert_with_key(|id| View::new(id, buf));
        Ok(())
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
            let (view, buf) = active!(self);
            let pos = view.cursor();
            let fut = server.definition(lsp_types::GotoDefinitionParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier { uri: buf.url() },
                    position: pos.into(),
                },
                work_done_progress_params: lsp_types::WorkDoneProgressParams {
                    work_done_token: None,
                },
                partial_result_params: lsp_types::PartialResultParams {
                    partial_result_token: None,
                },
            });

            self.tx
                .send(Box::pin(async move {
                    if let Ok(Some(res)) = fut.await {
                        return Box::new(move |editor: &mut Editor| match res {
                            lsp_types::GotoDefinitionResponse::Scalar(location) => todo!(),
                            // TODO
                            lsp_types::GotoDefinitionResponse::Array(_) => {}
                            lsp_types::GotoDefinitionResponse::Link(_) => {}
                        }) as Box<dyn FnOnce(&mut Editor)>;
                    }
                    Box::new(|_: &mut Editor| {})
                }))
                .expect("send failed");
        }
        todo!()
    }
}
