mod completion;

mod config;
pub(crate) mod cursor;
pub mod db;
mod default_keymap;
mod diagnostics;
mod dot;
mod errors;
mod events;
mod file_io;
mod lsp_requests;
mod marks;
mod pickers;
mod register;
mod render;
mod search;
mod state;
pub mod visual;

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::future::Future;
use std::ops::{self, Deref, Index, IndexMut};
use std::path::{Path, PathBuf};
use std::pin::{Pin, pin};
use std::sync::{Arc, OnceLock, Weak};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use std::{cmp, fmt, io, mem};

use anyhow::{anyhow, bail};
use arboard::Clipboard;
use futures_util::stream::FuturesUnordered;
use futures_util::{Stream, StreamExt};
use ignore::WalkState;
use stdx::path::{PathExt, Relative};
use tokio::select;
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tokio::sync::{Notify, oneshot};
use ustr::Ustr;
use zi_core::{PointOrByte, PointRange, Size};
use zi_indent::Indent;
use zi_input::{Event, KeyCode, KeyEvent, KeySequence};
use zi_text::{AnyText, Delta, Deltas, ReadonlyText, Rope, Text, TextSlice};
use zi_textobject::motion::{self, Motion, MotionFlags};
use zi_textobject::{TextObject, TextObjectFlags, TextObjectKind};

use self::config::Settings;
use self::db::{BufferQueries as _, Db, ViewQueries as _};
use self::diagnostics::BufferDiagnostics;
use self::dot::Dot;
pub use self::errors::EditError;
use self::file_io::{OpenFile, OpenPlan};
use self::register::Registers;
pub use self::register::{Register, RegisterKind};
pub use self::search::Match;
use self::search::SearchState;
use self::state::{OperatorPendingState, State};
use crate::buffer::picker::{BufferPicker, BufferPickerEntry, DynamicHandler, Picker};
use crate::buffer::{
    self as buffer, Buffer, BufferFlags, EditFlags, ExplorerBuffer, IndentSettings, Injector,
    InspectorBuffer, PickerBuffer, SnapshotFlags, TextBuffer,
};
use crate::command::{self, Command, CommandKind, Handler, Word};
use crate::completion::Completion;
use crate::event::EventHandler;
use crate::keymap::{DynKeymap, Keymap, TrieResult};
use crate::language_service::LanguageServiceInstance;
use crate::layout::Layer;
use crate::namespace::NamespaceQueries as _;
use crate::plugin::PluginManager;
use crate::syntax::{HighlightId, Syntax, Theme};
use crate::view::{SetCursorFlags, ViewGroup, ViewGroupQueries as _};
use crate::{
    BufferId, Direction, Error, FileType, LanguageService, LanguageServiceId, Location, Mode,
    Namespace, NamespaceId, Operator, Point, Result, Setting, VerticalAlignment, View, ViewGroupId,
    ViewId, event, filetype, language, layout,
};

bitflags::bitflags! {
    #[derive(Default, Clone, Copy, PartialEq, Eq)]
    pub struct OpenFlags: u32 {
        const NONE = 0;
        const READONLY = 1 << 0;
        const SPAWN_LANGUAGE_SERVICES = 1 << 1;
        /// Don't open the buffer in the active view
        const BACKGROUND = 1 << 2;
        /// Reload the buffer from disk even if it's already open discarding any unsaved changes.
        const FORCE = 1 << 3;
    }

    #[derive(Default, Clone, Copy, PartialEq, Eq)]
    pub struct SaveFlags: u32 {
        /// Flush the buffer to disk even if it's not dirty
        const FORCE = 1 << 0;
    }
}

// Ergonomic closure like clipboard access macro.
// This exists for two reasons:
// - We don't use a function so we can do partial borrows of `self`
// - We want to hide the `Arc` error handling boilerplate. The arc is necessary as the error is not clone.
macro_rules! with_clipboard {
    ($self:expr, |$cb:ident| $body:expr) => {
        match &mut $self.clipboard {
            Ok(cb) => {
                let $cb = cb;
                let res = $body;
                res.map_err(Arc::new)
            }
            Err(err) => Err(Arc::clone(err)),
        }
    };
}

fn pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| rayon::ThreadPoolBuilder::new().build().unwrap())
}

#[derive(Default)]
pub struct DummyBackend;

impl Backend for DummyBackend {
    fn new_syntax(&mut self, _ft: FileType) -> io::Result<Option<Box<dyn Syntax>>> {
        Ok(None)
    }
}

pub trait Backend: Send + Sync + 'static {
    fn new_syntax(&mut self, ft: FileType) -> io::Result<Option<Box<dyn Syntax>>>;
}

pub struct Editor {
    pub(crate) db: Db,
    active_language_services: HashMap<LanguageServiceId, LanguageServiceInstance>,
    language_config: language::Config,
    registers: Registers,
    default_namespace: NamespaceId,
    // We key diagnostics by `path` instead of `BufferId` as it is valid to send diagnostics for an unloaded buffer.
    // The per-buffer diagnostics are sorted by range.
    diagnostics: HashMap<PathBuf, BufferDiagnostics>,
    empty_buffer: BufferId,
    settings: Settings,
    search_state: SearchState,
    state: State,
    keymap: Keymap,
    active_language_services_by_ft: HashMap<FileType, Vec<LanguageServiceId>>,
    /// Services whose `initialize` is in flight; an entry is stale once its init future is gone.
    starting_language_services: HashMap<LanguageServiceId, Weak<()>>,
    callbacks_tx: CallbacksSender,
    requests_tx: tokio::sync::mpsc::Sender<Request>,
    tree: layout::ViewTree,
    /// error to be displayed in the status line
    status_error: Option<String>,
    command_handlers: HashMap<Word, Handler>,
    // plugins: Plugins,
    notify_quit: Notify,
    backend: Box<dyn Backend>,
    plugin_managers: BTreeMap<&'static str, Arc<dyn PluginManager + Send + Sync>>,
    clipboard: Result<Clipboard, Arc<arboard::Error>>,
    dot: Dot,
    count: Option<usize>,
}

macro_rules! mode {
    ($editor:ident) => {
        $editor.state.mode()
    };
}

pub(super) use mode;

impl Index<ViewId> for Editor {
    type Output = View;

    #[inline]
    fn index(&self, index: ViewId) -> &Self::Output {
        &self.db.views[index].view
    }
}

impl IndexMut<ViewId> for Editor {
    #[inline]
    fn index_mut(&mut self, index: ViewId) -> &mut Self::Output {
        self.db.view_mut(index)
    }
}

impl Index<ViewGroupId> for Editor {
    type Output = ViewGroup;

    #[inline]
    fn index(&self, index: ViewGroupId) -> &Self::Output {
        &self.db.view_groups[index]
    }
}

impl Index<NamespaceId> for Editor {
    type Output = Namespace;

    #[inline]
    fn index(&self, index: NamespaceId) -> &Self::Output {
        &self.db.namespaces[index]
    }
}

impl Index<BufferId> for Editor {
    type Output = Buffer;

    #[inline]
    fn index(&self, index: BufferId) -> &Self::Output {
        &self.db.buffers[index].buffer
    }
}

impl IndexMut<BufferId> for Editor {
    #[inline]
    fn index_mut(&mut self, index: BufferId) -> &mut Self::Output {
        self.db.buffer_mut(index)
    }
}

pub(crate) type Action = fn(&mut Editor);

static NOTIFY_REDRAW: OnceLock<Notify> = OnceLock::new();

fn request_redraw() {
    NOTIFY_REDRAW.get().expect("editor was not initialized").notify_one();
}

macro_rules! set_error {
    ($editor:ident, $error:expr) => {
        $editor.status_error = Some($error.to_string())
    };
}

use set_error;

macro_rules! set_error_if {
    ($editor:ident: $error:expr) => {
        if let Err(err) = $error {
            $crate::editor::set_error!($editor, err);
        }
    };
}

use set_error_if;

macro_rules! active_servers_of {
    ($editor:ident, $selector:expr) => {{
        let buf = $selector.select($editor);
        $editor
            .active_language_services_by_ft
            .get(&$editor.db.buffers[buf].file_type())
            .map_or(&[][..], |s| &s[..])
            .iter()
    }};
}

use active_servers_of;

pub(crate) type EditorCallback = Box<dyn FnOnce(&mut Editor) -> Result<(), Error> + Send>;

type CallbackFuture = Pin<Box<dyn Future<Output = Result<EditorCallback, Error>> + Send>>;

type CallbacksSender = UnboundedSender<CallbackFuture>;

// Adaptor for tokio's channel to be a futures Stream
struct ChannelStream<T>(Receiver<T>);

impl<T> Stream for ChannelStream<T> {
    type Item = T;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.poll_recv(cx)
    }
}

// Adaptor for tokio's unbounded channel to be a futures Stream
struct UnboundedChannelStream<T>(UnboundedReceiver<T>);

impl<T> Stream for UnboundedChannelStream<T> {
    type Item = T;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.poll_recv(cx)
    }
}

struct Request {
    #[expect(clippy::type_complexity, reason = "spelled out once, here")]
    f: Box<dyn FnOnce(&mut Editor) -> Box<dyn Any + Send> + Send>,
    tx: oneshot::Sender<Box<dyn Any + Send>>,
}

/// An async client to the editor.
#[derive(Clone)]
pub struct Client {
    requests_tx: Sender<Request>,
    callbacks_tx: CallbacksSender,
}

impl Client {
    pub async fn with<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Editor) -> T + Send + 'static,
    ) -> T {
        let (tx, rx) = oneshot::channel();
        self.requests_tx
            .send(Request { tx, f: Box::new(|editor| Box::new(f(editor))) })
            .await
            .expect("request receiver should be alive");
        *rx.await.expect("server did not send response").downcast().unwrap()
    }

    pub async fn spawn(
        &self,
        desc: &'static str,
        fut: impl Future<Output = Result<()>> + Send + 'static,
    ) {
        self.with(move |editor| editor.spawn(desc, fut)).await;
    }

    /// Send a callback to the editor to be executed.
    /// This is a sync operation with the limitation that we can't return a value.
    pub fn send(&self, f: impl FnOnce(&mut Editor) -> Result<()> + Send + 'static) {
        // no description needed as `ready()` will never timeout
        callback(&self.callbacks_tx, "", std::future::ready(Ok(())), |editor, ()| f(editor));
    }
}

pub struct Tasks {
    requests: ChannelStream<Request>,
    callbacks: UnboundedChannelStream<CallbackFuture>,
    notify_redraw: &'static Notify,
}

impl Editor {
    // status line + command line
    pub const BOTTOM_BAR_HEIGHT: u16 = 1 + 1;

    /// Create a new editor with a scratch buffer.
    /// Returns the editor instance and a stream of callbacks.
    /// The callback stream must be polled and the resulting callback executed on the editor.
    /// The `notify` instance is used to signal the main thread to redraw the screen.
    /// It is recommended to implement a debounce mechanism to avoid redrawing too often.
    pub fn new(backend: impl Backend, size: impl Into<Size>) -> (Self, Tasks) {
        let size = size.into();
        let settings = Settings::default();
        let mut db = Db::default();
        let theme = settings.theme.read();
        let scratch_buffer = db.insert_buffer(|id| {
            Buffer::new(TextBuffer::new(
                id,
                Default::default(),
                filetype!(text),
                "scratch",
                Rope::new(),
                &theme,
                None,
            ))
        });
        let active_view = db.insert_view(scratch_buffer, None, View::new);

        let default_namespace =
            rel::Database::insert_with(&mut db, |id| Namespace::new(id, "default"))
                .expect("the db has no namespaces yet");

        let empty_buffer = db.insert_buffer(|id| {
            Buffer::new(TextBuffer::new(
                id,
                buffer::Settings { readonly: Setting::new(true), ..Default::default() },
                filetype!(text),
                "empty",
                "",
                &theme,
                None,
            ))
        });

        // Using an unbounded channel as we need `callbacks_tx.send()` to be sync.
        let (callbacks_tx, callbacks_rx) = tokio::sync::mpsc::unbounded_channel();

        let (requests_tx, requests_rx) = tokio::sync::mpsc::channel(128);
        // let plugins = Plugins::new(Client {
        //     requests_tx: requests_tx.clone(),
        //     callbacks_tx: callbacks_tx.clone(),
        // });

        drop(theme);
        let mut editor = Self {
            db,
            default_namespace,
            callbacks_tx,
            requests_tx,
            // plugins,
            empty_buffer,
            settings,
            clipboard: Clipboard::new().map_err(Arc::new),
            backend: Box::new(backend),
            keymap: default_keymap::new(),
            tree: layout::ViewTree::new(size, active_view),
            command_handlers: command::builtin_handlers(),
            registers: Default::default(),
            diagnostics: Default::default(),
            notify_quit: Default::default(),
            language_config: Default::default(),
            active_language_services: Default::default(),
            active_language_services_by_ft: Default::default(),
            starting_language_services: Default::default(),
            state: Default::default(),
            search_state: Default::default(),
            status_error: Default::default(),
            plugin_managers: Default::default(),
            dot: Default::default(),
            count: None,
        };

        let notify_redraw = NOTIFY_REDRAW.get_or_init(Default::default);
        editor.resize(size);
        Self::subscribe_sync_hooks();

        (
            editor,
            Tasks {
                requests: ChannelStream(requests_rx),
                callbacks: UnboundedChannelStream(callbacks_rx),
                notify_redraw,
            },
        )
    }

    pub fn language_service(
        &mut self,
        id: LanguageServiceId,
    ) -> Option<&mut (dyn LanguageService + Send + 'static)> {
        self.active_language_services.get_mut(&id).map(|s| &mut **s)
    }

    pub fn register_plugin_manager(
        &mut self,
        manager: impl PluginManager + Send + Sync + 'static,
    ) -> Option<Arc<dyn PluginManager + Send + Sync>> {
        self.plugin_managers.insert(manager.name(), Arc::new(manager))
    }

    pub fn client(&self) -> Client {
        Client { requests_tx: self.requests_tx.clone(), callbacks_tx: self.callbacks_tx.clone() }
    }

    pub fn size(&self) -> Size {
        self.tree.size()
    }

    fn check_open(path: &mut PathBuf, open_flags: OpenFlags) -> io::Result<()> {
        use std::io::Read;

        if path.exists() && !path.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
        }

        if !path.exists() && open_flags.contains(OpenFlags::READONLY) {
            return Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        }

        if path.exists() {
            // Try ensure that the file does not contains non-utf8 data.
            let mut buf = [0u8; 1024];
            let n = File::open(path.as_path())?.read(&mut buf)?;
            match content_inspector::inspect(&buf[..n]) {
                content_inspector::ContentType::UTF_8
                | content_inspector::ContentType::UTF_8_BOM => {}
                content_inspector::ContentType::BINARY
                | content_inspector::ContentType::UTF_16LE
                | content_inspector::ContentType::UTF_16BE
                | content_inspector::ContentType::UTF_32LE
                | content_inspector::ContentType::UTF_32BE => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "non-utf8 data"));
                }
            }

            *path = path.canonicalize()?;
        }

        Ok(())
    }

    fn buffer_at_path(&self, path: &Path) -> Option<BufferId> {
        self.db.buffers.by_path(&Some(path.to_path_buf())).next().map(|b| b.id())
    }

    pub fn open(
        &mut self,
        path: impl AsRef<Path>,
        open_flags: OpenFlags,
    ) -> io::Result<impl Future<Output = Result<BufferId>> + 'static> {
        let theme = self.theme();
        let mut path = path.as_ref().to_path_buf();
        Self::check_open(&mut path, open_flags)?;

        let ft = FileType::detect(&path);
        let syntax = self.backend.new_syntax(ft)?;

        let plan = self.plan_open(&path, open_flags);
        let client = self.client();
        Ok(async move {
            let start = Instant::now();
            let buf = match plan {
                OpenPlan::Reuse(buf) => buf,
                OpenPlan::Load { replace } => {
                    let file = OpenFile { path: path.clone(), ft, syntax, theme };
                    file.load(&client, replace, open_flags).await?
                }
            };
            client.with(move |editor| editor.finish_open(buf, ft, open_flags)).await?;
            tracing::debug!(?path, %ft, time = ?start.elapsed(), "opened buffer");
            Ok(buf)
        })
    }

    fn plan_open(&self, path: &Path, flags: OpenFlags) -> OpenPlan {
        let Some(buf) = self.buffer_at_path(path) else {
            return OpenPlan::Load { replace: None };
        };

        // Reopen readonly buffers in place when the caller requests a writable one.
        if *self[buf].settings().readonly.read() && !flags.contains(OpenFlags::READONLY)
            || flags.contains(OpenFlags::FORCE)
        {
            OpenPlan::Load { replace: Some(buf) }
        } else {
            OpenPlan::Reuse(buf)
        }
    }

    fn finish_open(&mut self, buf: BufferId, ft: FileType, flags: OpenFlags) -> Result<()> {
        if !self.has_buffer(buf) {
            bail!("buffer {buf:?} was removed while it was opening");
        }
        if !flags.contains(OpenFlags::BACKGROUND) {
            self.set_buffer(Active, buf);
        }
        if flags.contains(OpenFlags::SPAWN_LANGUAGE_SERVICES) {
            self.spawn_language_services_for_ft(ft)?;
        }
        self.dispatch(event::DidOpenBuffer { buf });
        Ok(())
    }

    pub fn register_command(&mut self, handler: Handler) -> Option<Handler> {
        self.command_handlers.insert(handler.name(), handler)
    }

    pub(crate) fn empty_buffer(&self) -> BufferId {
        self.empty_buffer
    }

    pub fn default_namespace(&self) -> NamespaceId {
        self.default_namespace
    }

    pub fn set_buffer(&mut self, view: impl Selector<ViewId>, buf: impl Selector<BufferId>) {
        let view = view.select(self);
        let buf = buf.select(self);
        let prev = self.db.views[view].buffer();
        self.db.set_buffer(view, buf);
        self.wipe_if_hidden(prev);
    }

    pub fn highlight_id_by_name(&self, name: impl AsRef<str>) -> HighlightId {
        self.theme().read().highlight_id_by_name(name)
    }

    async fn shutdown(&mut self) {
        for mut server in mem::take(&mut self.active_language_services).into_values() {
            // TODO shutdown concurrently
            if let Err(err) = server.shutdown().await {
                tracing::error!(error = &*err, "language server shutdown failed");
            }

            if let Err(err) = server.wait().await {
                tracing::error!(error = &*err, "language server wait failed: {err}");
            }
        }
    }

    fn should_quit(&self) -> bool {
        if self.tree.is_empty() {
            self.notify_quit.notify_one();
            return true;
        }

        false
    }

    pub fn get_error(&mut self) -> Option<&str> {
        self.status_error.as_deref()
    }

    pub fn set_error(&mut self, error: impl fmt::Display) {
        // TODO push all the corresponding tracing error in here
        set_error!(self, error);
    }

    #[doc(hidden)]
    pub fn cursor_viewport_coords(&self) -> (u16, u16) {
        if mode!(self) == Mode::Command {
            return (1, self.tree.area().height + 1);
        }

        let (view, buf) = self.db.view_and_buffer(self.tree.active());
        let area = self.tree.view_area(view.id());
        let (x, y) = view.cursor_viewport_coords(buf);
        (x + area.x, y + area.y)
    }

    pub fn input<S>(&mut self, seq: S) -> Result<(), S::Error>
    where
        S: TryInto<KeySequence>,
    {
        for key in seq.try_into()? {
            self.handle_input(Event::Key(key));
        }

        Ok(())
    }

    pub fn handle_input(&mut self, event: impl Into<Event>) {
        match event.into() {
            Event::Key(key) => self.handle_key_event(key),
            Event::Resize(width, height) => self.resize(Size::new(width, height)),
        }
    }

    pub(crate) fn tree(&self) -> &layout::ViewTree {
        &self.tree
    }

    fn resize(&mut self, size: Size) {
        assert!(size.height > Self::BOTTOM_BAR_HEIGHT, "height must be at least 3");
        // Subtract 2 from the height to leave room for the status line and command line.
        let size = Size { height: size.height - Self::BOTTOM_BAR_HEIGHT, ..size };
        self.tree.resize(size);
        request_redraw();
    }

    #[doc(hidden)]
    pub async fn test_run(mut self, tasks: Tasks) -> io::Result<()> {
        self.run(futures_util::stream::empty(), tasks, |_| Ok(())).await
    }

    pub async fn run(
        &mut self,
        events: impl Stream<Item = io::Result<Event>>,
        Tasks { requests, callbacks, notify_redraw }: Tasks,
        mut render: impl FnMut(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        Self::subscribe_async_hooks().await;

        render(self)?;

        let mut plugin_manager_handles = self
            .plugin_managers
            .values()
            .cloned()
            .map(|m| tokio::spawn(m.start(self.client())))
            .collect::<FuturesUnordered<_>>()
            .fuse();

        let mut requests = pin!(requests.fuse().peekable());
        let mut callbacks = pin!(callbacks.buffer_unordered(128).peekable());

        let mut events = pin!(events);
        loop {
            select! {
                biased;
                Some(event) = events.next() => self.handle_input(event?),
                () = notify_redraw.notified() => tracing::trace!("redrawing due to request"),
                result = callbacks.select_next_some() => self.handle_callback_result(result),
                req = requests.select_next_some() => {
                    // If the receiver dropped then we just ignore the request.
                    let _ = req.tx.send((req.f)(self));
                },
                Some(result) = plugin_manager_handles.next() => self.handle_plugin_exit(result),
                // Put the quit case last to ensure we handle all events first
                () = self.notify_quit.notified() => break,
            }

            // Don't immediately break here as we want to finish handling any events first
            if self.should_quit() {
                // TODO if we don't break here some assertions fail
                break;
            }

            render(self)?;
        }

        self.shutdown().await;

        Ok(())
    }

    fn handle_callback_result(&mut self, result: Result<EditorCallback>) {
        match result {
            Ok(callback) => {
                if let Err(err) = callback(self) {
                    tracing::error!(error = &*err, "task callback failed");
                    self.set_error(err);
                }
            }
            Err(err) => {
                tracing::error!(error = &*err, "task failed");
                self.set_error(err);
            }
        }
    }

    fn handle_plugin_exit(&mut self, result: Result<Result<()>, tokio::task::JoinError>) {
        match result {
            Ok(Ok(())) => (),
            Ok(Err(err)) => {
                tracing::error!(error = &*err, "plugin manager failed");
                self.set_error(err);
            }
            Err(err) => {
                tracing::error!(error = &err as &dyn std::error::Error, "plugin manager died");
                self.set_error(err);
            }
        }
    }

    fn update_search(&mut self) {
        let view = self.tree().active();
        let active_buffer = self.view(view).buffer();

        let State::Command(state) = &mut self.state else { return };

        let (k, query) = state.buffer().split_at(1);
        match k {
            ":" => {}
            "/" => {
                use regex_cursor::engines::meta::Regex;

                if query.is_empty() {
                    return;
                }

                if !self.search_state.prepare_update(active_buffer, query) {
                    return;
                }

                let regex = match Regex::new(query) {
                    Ok(regex) => regex,
                    Err(err) => return set_error!(self, err),
                };

                let (view, buf) = self.db.view_and_buffer_mut(self.tree.active());

                let text = buf.text();
                self.search_state.set_matches(
                    text.point_to_byte(view.cursor()),
                    search::scan_buffer(text, &regex),
                );

                self.goto_match(|s| s.current_match());
            }
            _ => unreachable!(),
        }
    }

    fn handle_insert(&mut self, c: char) -> Result<(), EditError> {
        match &mut self.state {
            State::Insert(..) => self.insert_char(Active, c),
            State::Command(state) => {
                state.buffer.push(c);
                self.update_search();
                Ok(())
            }
            State::Normal(_)
            | State::Visual(_)
            | State::VisualLine(_)
            | State::VisualBlock(_)
            | State::OperatorPending(_)
            | State::ReplacePending => unreachable!(),
        }
    }

    #[inline]
    fn handle_key_event(&mut self, key: KeyEvent) {
        self.status_error = None;
        let mode = mode!(self);

        self.dot.record_input(mode, &key);

        tracing::trace!(%key, "handling key");
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "every other key goes through the keymap"
        )]
        match key.code() {
            KeyCode::Char(c) if matches!(mode, Mode::ReplacePending) => {
                self.replace_pending_char(c);
            }
            KeyCode::Char(_) if matches!(mode, Mode::Insert | Mode::Command) => {
                self.handle_text_key(mode, key);
            }
            _ => self.handle_command_key(mode, key),
        }
    }

    fn replace_pending_char(&mut self, c: char) {
        self.set_mode(Mode::Insert);
        self.move_cursor(Active, Direction::Right, 1);
        let _ = self.delete_char(Active);
        let _ = self.insert_char(Active, c);
        self.set_mode(Mode::Normal);
    }

    fn resolve_key(&mut self, mode: Mode, key: KeyEvent) -> (TrieResult<Action>, Vec<KeyEvent>) {
        let mut empty = Keymap::default();
        let (_, buf) = self.db.view_and_buffer_mut(self.tree.active());
        self.keymap.pair(buf.keymap().unwrap_or(&mut empty)).on_key(mode, key)
    }

    fn handle_text_key(&mut self, mode: Mode, key: KeyEvent) {
        let (result, buffered) = self.resolve_key(mode, key);
        if let TrieResult::Found(action) = result {
            action(self);
        }

        for event in buffered {
            let KeyCode::Char(c) = event.code() else { unreachable!() };
            if let Err(err) = self.handle_insert(c) {
                set_error!(self, err);
            }
        }
    }

    fn handle_command_key(&mut self, mode: Mode, key: KeyEvent) {
        match self.resolve_key(mode, key).0 {
            TrieResult::Found(action) => self.run_key_action(mode, action),
            TrieResult::Partial => (),
            TrieResult::Nothing => {
                self.count = None;
                if matches!(mode, Mode::OperatorPending(_) | Mode::ReplacePending) {
                    self.set_mode(Mode::Normal);
                }
            }
        }
    }

    fn run_key_action(&mut self, mode: Mode, action: Action) {
        let count = self.count;
        self.dot.start_key();
        action(self);
        if self.count == count && !matches!(mode!(self), Mode::OperatorPending(_)) {
            self.count = None;
        }

        if let Some(new) = self.count.filter(|_| self.count != count) {
            self.dot.save_count(mode, new);
        }

        if mode == Mode::Normal && mode!(self) == Mode::Normal {
            self.dot.finalize_normal_mode_change();
            if self.count.is_none() {
                self.dot.clear_normal_keys();
            }
        }
    }

    #[inline]
    pub fn mode(&self) -> Mode {
        mode!(self)
    }

    pub(crate) fn take_count(&mut self) -> Option<usize> {
        self.count.take()
    }

    pub(crate) fn update_count(&mut self, f: impl FnOnce(Option<usize>) -> usize) {
        self.count = Some(f(self.count));
    }

    pub fn visual_anchor(&self) -> Option<Point> {
        self.state.visual_anchor()
    }

    pub fn visual_selection(&self, selector: impl Selector<ViewId>) -> Option<visual::Selection> {
        let anchor = self.state.visual_anchor()?;
        let view = selector.select(self);
        let cursor = self[view].cursor();
        let (start, end) = if anchor <= cursor { (anchor, cursor) } else { (cursor, anchor) };

        match mode!(self) {
            Mode::Visual => Some(visual::Selection::Charwise { start, end }),
            Mode::VisualLine => {
                Some(visual::Selection::Line { start_line: start.line(), end_line: end.line() })
            }
            Mode::VisualBlock => {
                Some(visual::Selection::block(&self[self.view(view).buffer()], anchor, cursor))
            }
            Mode::Normal
            | Mode::Insert
            | Mode::Command
            | Mode::OperatorPending(_)
            | Mode::ReplacePending => None,
        }
    }

    /// Replay the last change (dot repeat)
    pub fn dot_repeat(&mut self) {
        // Collect the events to replay (we can't borrow self.dot while replaying)
        let events: Vec<_> = self.dot.events().to_vec();

        if events.is_empty() {
            return;
        }

        if let Some(count) = self.take_count() {
            self.dot.set_count(count);
        }
        self.count = self.dot.count();

        // Mark that we're replaying to prevent recording during the replay
        self.dot.start_replaying();

        // Replay each recorded event
        for event in events {
            self.handle_key_event(event);
        }

        // Stop replaying
        self.dot.stop_replaying();
    }

    pub fn execute<C>(&mut self, cmd: C) -> crate::Result<()>
    where
        C: TryInto<Command>,
        Error: From<<C as TryInto<Command>>::Error>,
    {
        let cmd: Command = cmd.try_into()?;
        let range = cmd.range();
        match cmd.kind() {
            CommandKind::Generic { cmd, args, force } => {
                if let Some(handler) = self.command_handlers.get(cmd) {
                    handler.execute(self, range.cloned(), args.clone(), *force)?;
                } else {
                    anyhow::bail!("unknown command: {cmd}")
                }
            }
        }

        Ok(())
    }

    fn execute_buffered_command(&mut self) -> Result<()> {
        let State::Command(state) = &mut self.state else { return Ok(()) };

        if state.buffer.starts_with('/') {
            self.set_mode(Mode::Normal);
            return Ok(());
        }

        let Some(cmd) = state.buffer.strip_prefix(':') else {
            bail!("command must start with `:`")
        };

        let cmd = cmd.parse::<Command>();
        state.buffer.clear();
        self.set_mode(Mode::Normal);
        self.execute(cmd?)?;

        Ok(())
    }

    #[inline]
    pub fn set_mode(&mut self, to: Mode) {
        let from = mode!(self);

        if let (Mode::Insert, Mode::Normal) = (from, to) {
            self.insert_to_normal();
        }

        self.dispatch(event::WillChangeMode { from, to });
        self.state = State::new(self, to);

        if !self.dot.is_replaying() {
            if Dot::should_start_recording(from, to) {
                self.dot.start_recording();
            } else if Dot::should_stop_recording(from, to) {
                self.dot.stop_recording();
            }
        }

        self.dispatch(event::DidChangeMode { from, to });
    }

    fn insert_to_normal(&mut self) {
        assert_eq!(self.mode(), Mode::Insert);
        let (view, buf) = self.get(Active);

        {
            // Clear any whitespace at the end of the cursor line when exiting insert mode
            let cursor = self[view].cursor();
            if let Some(range) = self[buf].text().line(cursor.line()).and_then(|line| {
                let end = Point::new(cursor.line(), line.len_bytes());
                let mut start = end;
                for c in line.chars().rev() {
                    if !c.is_whitespace() {
                        break;
                    }

                    start = start.left_bytes(c.len_utf8());
                }

                (start != end).then(|| PointRange::new(start, end))
            }) {
                let byte_range = self[buf].text().point_range_to_byte_range(range);
                let _ = self.edit(Active, &Deltas::delete(byte_range));
            }
        }

        self[buf].snapshot(SnapshotFlags::empty());
        // Move cursor left when exiting insert mode
        let _ = self.motion(Active, motion::PrevChar);

        // Request diagnostics using the pull model to ensure we have the latest diagnostics
        let fut = self.request_diagnostics(Active);
        self.spawn("pull diagnostics", fut);
    }

    #[inline]
    pub fn view(&self, selector: impl Selector<ViewId>) -> &crate::db::View {
        self.db.views.get(selector.select(self)).expect("bad view id")
    }

    #[inline]
    pub(crate) fn view_mut(&mut self, selector: impl Selector<ViewId>) -> &mut View {
        self.db.view_mut(selector.select(self))
    }

    pub fn text(&self, selector: impl Selector<BufferId>) -> &dyn AnyText {
        self.buffer(selector).text()
    }

    #[inline]
    pub fn buffer(&self, selector: impl Selector<BufferId>) -> &Buffer {
        &self.db.buffers.get(selector.select(self)).expect("bad buffer id").buffer
    }

    #[inline]
    pub fn has_view(&self, view: ViewId) -> bool {
        self.db.views.contains(view)
    }

    #[inline]
    pub fn has_buffer(&self, buf: BufferId) -> bool {
        self.db.buffers.contains(buf)
    }

    #[inline]
    pub(crate) fn buffer_mut(&mut self, selector: impl Selector<BufferId>) -> &mut Buffer {
        self.db.buffer_mut(selector.select(self))
    }

    #[inline]
    pub fn buffers(&self) -> impl Iterator<Item = &Buffer> {
        self.db.buffers.iter().map(|row| &row.row().buffer)
    }

    pub fn set_view_group(&mut self, selector: impl Selector<ViewId>, group: ViewGroupId) {
        let view = selector.select(self);
        self.db.set_group(view, group);
    }

    /// An iterator over all views in the view tree.
    // Note: this is not the same as `self.db.views.values()`
    #[inline]
    pub fn views(&self) -> impl Iterator<Item = &crate::db::View> {
        self.tree.views().map(move |id| self.view(id))
    }

    #[inline]
    pub fn view_only(&mut self, view: ViewId) {
        self.tree.view_only(view);
    }

    pub fn split(
        &mut self,
        selector: impl Selector<ViewId>,
        direction: Direction,
        constraint: tui::Constraint,
    ) -> ViewId {
        let view_id = selector.select(self);
        let row = self.view(view_id);
        let (id, buf, view) = (row.id(), row.buffer(), row.view.clone());
        let split_view = self.db.insert_view(buf, None, |id| View::split_from(id, view));
        self.tree.split(id, split_view, direction, constraint);
        split_view
    }

    pub fn focus(&mut self, selector: impl Selector<ViewId>) {
        let id = selector.select(self);
        self.tree.focus(id);
    }

    pub fn focus_direction(&mut self, direction: Direction) -> ViewId {
        self.tree.focus_direction(direction)
    }

    pub fn repeat_last_insert(&mut self) -> Result<(), EditError> {
        for kev in self.dot.events().to_vec() {
            self.handle_key_event(kev);
        }

        Ok(())
    }

    // Bad API used in tests for now
    #[doc(hidden)]
    pub fn delete_char(&mut self, selector: impl Selector<ViewId>) -> Result<(), EditError> {
        match &mut self.state {
            State::Command(state) => {
                state.buffer.pop();
                if state.buffer.is_empty() {
                    self.set_mode(Mode::Normal);
                }
                self.update_search();
                Ok(())
            }
            State::Normal(_)
            | State::Insert(_)
            | State::Visual(_)
            | State::VisualLine(_)
            | State::VisualBlock(_)
            | State::OperatorPending(_)
            | State::ReplacePending => {
                let view = selector.select(self);
                let (view, buf) = self.db.view_and_buffer_mut(view);
                if *buf.settings().readonly.read() {
                    return Err(EditError::Readonly);
                }

                let cursor = view.cursor();
                let text = buf.text();
                let byte_idx = text.point_to_byte(cursor);
                let Some(grapheme) = zi_text::grapheme::graphemes_before(text, byte_idx).next()
                else {
                    return Ok(());
                };
                let start_byte_idx = byte_idx
                    .checked_sub(grapheme.len)
                    .expect("just checked there's a grapheme here");

                buf.edit(&Deltas::delete(start_byte_idx..byte_idx));

                view.set_cursor_bytewise(
                    mode!(self),
                    self.tree.view_area(view.id()),
                    buf,
                    start_byte_idx,
                    SetCursorFlags::empty(),
                );

                let view = view.id();
                self.dispatch(event::DidDeleteChar { view });

                Ok(())
            }
        }
    }

    pub fn insert_char(
        &mut self,
        selector: impl Selector<ViewId>,
        c: char,
    ) -> Result<(), EditError> {
        let mut cbuf = [0; 4];
        let view = self.view(selector);
        let cursor = view.cursor();

        let cursor_byte = self[view.buffer()].text().point_to_byte(cursor);
        let view_id = view.id();
        self.edit(view_id, &Deltas::insert_at(cursor_byte, &*c.encode_utf8(&mut cbuf)))?;

        let (view, buf) = self.db.view_and_buffer_mut(self.tree.active());
        let area = self.tree.view_area(view_id);
        match c {
            '\n' => {
                let cursor = view.move_cursor(mode!(self), area, buf, Direction::Down, 1);
                self.indent_newline(view_id)?;
                cursor
            }
            _ => self.motion(Active, motion::NextChar)?,
        };

        self.dispatch(event::DidInsertChar { view: view_id, char: c });

        Ok(())
    }

    pub fn tab(&mut self) -> Result<(), EditError> {
        match &mut self.state {
            State::Normal(..) => {
                // TODO
                Ok(())
            }
            State::Insert(state) => {
                if let Completion::Active(state) = &mut state.completion {
                    if let Some(delta) = state.select_next() {
                        self.apply_completion_delta(&delta)?;
                    }
                } else {
                    let (view, buf) = self.get(Active);
                    let indent = *self[buf].settings().indent.read();
                    match indent {
                        // Should probably align to a multiple of `n`
                        IndentSettings::Spaces(n) => self.insert(view, &" ".repeat(n as usize))?,
                        IndentSettings::Tabs => self.insert_char(view, '\t')?,
                    }
                }
                Ok(())
            }
            // TODO
            State::Visual(..)
            | State::VisualLine(..)
            | State::VisualBlock(..)
            | State::Command(..)
            | State::OperatorPending(_)
            | State::ReplacePending => Ok(()),
        }
    }

    pub fn backtab(&mut self) -> Result<(), EditError> {
        match &mut self.state {
            State::Normal(..) => {
                // TODO
                Ok(())
            }
            State::Insert(state) => {
                if let Completion::Active(state) = &mut state.completion {
                    if let Some(delta) = state.select_prev() {
                        self.apply_completion_delta(&delta)?;
                    }
                } else {
                    // TODO
                }
                Ok(())
            }
            // TODO
            State::Visual(..)
            | State::VisualLine(..)
            | State::VisualBlock(..)
            | State::Command(..)
            | State::OperatorPending(_)
            | State::ReplacePending => Ok(()),
        }
    }

    fn indent_newline(&mut self, selector: impl Selector<ViewId>) -> Result<(), EditError> {
        let (view, buf) = self.get(selector);
        let text = self[buf].text();
        let cursor = self[view].cursor();
        if cursor.byte_col() > 0 || !text.line(cursor.line()).unwrap().is_empty() {
            // Simple heuristics to determine if we should indent the new line
            return Ok(());
        }

        let tab_width = *self[buf].settings().tab_width.read();
        match zi_indent::indent(zi_indent::Config { tab_width }, text, cursor.line()) {
            Indent::Bytes(bytes) => {
                let start_byte = text.line_to_byte(cursor.line());
                let deltas = Deltas::insert_at(start_byte, " ".repeat(bytes));
                self.edit(buf, &deltas)?;
                self.set_cursor(view, cursor.right_bytes(bytes));
                Ok(())
            }
        }
    }

    fn get(&self, selector: impl Selector<ViewId>) -> (ViewId, BufferId) {
        let view = selector.select(self);
        let buf = self.view(view).buffer();
        (view, buf)
    }

    pub fn edit(
        &mut self,
        selector: impl Selector<BufferId>,
        deltas: &Deltas<'_>,
    ) -> Result<(), EditError> {
        self.edit_flags(selector, deltas, EditFlags::empty())
    }

    // This is a very naive implementation of a random delta generator.
    #[doc(hidden)]
    #[cfg(feature = "rand")]
    pub fn apply_random_deltas<R: rand::Rng + Clone>(
        &mut self,
        mut rng: R,
        selector: impl Selector<BufferId>,
    ) -> Deltas<'static> {
        use rand::RngExt as _;
        use zi_text::Delta;

        let buf = selector.select(self);

        // Set the string to something random otherwise only trivial deltas can be generated.
        let deltas = Deltas::insert_at(0, random_string(rng.clone(), 1000));
        self.edit(buf, &deltas).expect("initial random delta was invalid");

        // TODO should include some special characters like newlines and tabs
        fn random_string(mut rng: impl rand::Rng, n: usize) -> String {
            let len = rng.random_range(0..=n);
            let chars = rng.sample_iter(rand::distr::Alphanumeric);
            chars.take(len).map(|u| u as char).collect()
        }

        let len = self[buf].text().len_bytes();

        let n = rng.random_range(0..=10);
        let mut k = 0;
        let deltas = Deltas::new((0..n).map_while(|_| {
            if k > len {
                return None;
            }

            let start = if k == len { k } else { rng.random_range(k..=len) };
            let end = if start == len { start } else { rng.random_range(start..=len) };
            k = end + 1;
            let range = start..end;

            if rng.random_bool(0.1) {
                return None;
            }

            Some(Delta::new(range, random_string(rng.clone(), 100)))
        }));

        self.edit(buf, &deltas).expect("random delta was invalid");
        deltas
    }

    fn edit_flags(
        &mut self,
        selector: impl Selector<BufferId>,
        deltas: &Deltas<'_>,
        flags: EditFlags,
    ) -> Result<(), EditError> {
        let buf = selector.select(self);
        // Don't care if we're actually in insert mode, that's more a key binding namespace.

        if *self[buf].settings().readonly.read() {
            return Err(EditError::Readonly);
        }

        if !flags.contains(EditFlags::NO_RECORD) {
            self.dot.did_edit();
        }

        // Ensure the buffer ends with a newline before performing an insert.
        if *self[buf].settings().fix_eol.read()
            && !flags.contains(EditFlags::NO_ENSURE_TRAILING_NEWLINE)
            && deltas.has_inserts()
            && self[buf].text().chars().next_back() != Some('\n')
        {
            // This edit won't affect the primary delta so we don't need to adjust it.
            let len = self[buf].text().len_bytes();
            let newline_deltas = Deltas::insert_at(len, "\n");
            let old_text = dyn_clone::clone_box(self[buf].text());
            self[buf].edit(&newline_deltas);
            self.dispatch(event::DidChangeBuffer { buf, old_text, deltas: newline_deltas });
        }

        let old_text = dyn_clone::clone_box(self[buf].text());
        self[buf].edit_flags(deltas, flags);

        // set the cursor again in relevant views as it may be out of bounds after the edit
        for view in self.views_into_buf(buf) {
            let cursor = self[view].cursor();
            self.set_cursor_flags(view, cursor, SetCursorFlags::NO_FORCE_UPDATE_TARGET);
        }

        self.dispatch(event::DidChangeBuffer { buf, old_text, deltas: deltas.to_owned() });
        Ok(())
    }

    fn views_into_buf(&self, buf: BufferId) -> impl Iterator<Item = ViewId> + 'static {
        self.tree
            .views()
            .filter(move |&view| self.view(view).buffer() == buf)
            // Collect first to avoid borrowing self
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn dispatch(&mut self, event: impl event::Event) {
        event::dispatch(self, event);
    }

    pub fn insert(&mut self, selector: impl Selector<ViewId>, text: &str) -> Result<(), EditError> {
        let view = selector.select(self);
        for c in text.chars() {
            self.insert_char(view, c)?;
        }
        Ok(())
    }

    // This and `cursor_char` won't make sense with visual mode
    // Used for tests for now
    #[doc(hidden)]
    pub fn cursor_line(&self) -> String {
        let (view, buffer) = self.db.view_and_buffer(self.tree.active());
        let cursor = view.cursor();
        let text = buffer.text();
        let line = text.line(cursor.line()).unwrap_or_else(|| Box::new(""));
        line.to_string()
    }

    #[doc(hidden)]
    pub fn cursor_char(&self) -> Option<char> {
        let (view, _) = self.db.view_and_buffer(self.tree.active());
        let cursor = view.cursor();
        let col = cursor.byte_col();
        self.cursor_line().get(col..)?.chars().next()
    }

    pub fn theme(&self) -> Setting<Theme> {
        self.settings().theme.share()
    }

    pub fn visual_op(&mut self, operator: Operator, selector: impl Selector<ViewId> + Copy) {
        let Some(sel) = self.visual_selection(selector) else { return };
        let view = selector.select(self);
        let buf = self.view(view).buffer();
        let content = sel.content(self[buf].text());
        let kind = sel.register_kind();

        if let Err(err) = with_clipboard!(self, |cb| cb.set_text(content.clone())) {
            set_error!(self, err);
        }
        self.registers.get_or_insert(Registers::UNNAMED).set(kind, content);

        if matches!(operator, Operator::Delete | Operator::Change) {
            let byte_ranges = sel.byte_ranges(self[buf].text());
            let start_point = sel.start_point();

            if operator == Operator::Change {
                self[buf].snapshot_cursor(start_point);
                self[buf].snapshot(SnapshotFlags::empty());
            }

            let is_linewise_change =
                operator == Operator::Change && matches!(sel, visual::Selection::Line { .. });
            for range in &byte_ranges {
                let deltas = if is_linewise_change {
                    Deltas::new([Delta::new(range.clone(), "\n")])
                } else {
                    Deltas::delete(range.clone())
                };
                if let Err(err) = self.edit(view, &deltas) {
                    set_error!(self, err);
                    return;
                }
            }

            let target_mode =
                if operator == Operator::Change { Mode::Insert } else { Mode::Normal };
            let (view, buf) = self.db.view_and_buffer_mut(view);
            if operator == Operator::Delete {
                buf.snapshot_cursor(start_point);
                buf.snapshot(SnapshotFlags::empty());
            }
            let area = self.tree.view_area(view.id());
            view.set_cursor_bytewise(
                target_mode,
                area,
                buf,
                buf.text().point_to_byte(start_point),
                SetCursorFlags::empty(),
            );
            self.set_mode(target_mode);
        } else {
            let start_point = sel.start_point();
            let (view, buf) = self.db.view_and_buffer_mut(view);
            let area = self.tree.view_area(view.id());
            view.set_cursor_bytewise(
                Mode::Normal,
                area,
                buf,
                buf.text().point_to_byte(start_point),
                SetCursorFlags::empty(),
            );
            self.set_mode(Mode::Normal);
        }
    }

    pub fn visual_yank(&mut self, selector: impl Selector<ViewId> + Copy) {
        self.visual_op(Operator::Yank, selector);
    }

    pub fn visual_delete(&mut self, selector: impl Selector<ViewId> + Copy) {
        self.visual_op(Operator::Delete, selector);
    }

    pub fn visual_change(&mut self, selector: impl Selector<ViewId> + Copy) {
        self.visual_op(Operator::Change, selector);
    }

    pub fn register(&self, name: char) -> Option<&register::Register> {
        self.registers.get(name)
    }

    pub fn paste_after(&mut self, selector: impl Selector<ViewId>) -> Result<(), EditError> {
        // FIXME very naive implementation.
        let Some(reg) = self.registers.get(Registers::UNNAMED).cloned() else {
            return Ok(());
        };

        match reg.kind {
            register::RegisterKind::Charwise => self.insert(selector, &reg.content),
            register::RegisterKind::Linewise => {
                let (view, buf) = self.get(selector);
                let cursor = self[view].cursor();
                let text = self[buf].text();
                let line_start_byte = text.line_to_byte(cursor.line() + 1).min(text.len_bytes());
                let deltas =
                    Deltas::new([Delta::insert_at(line_start_byte, format!("{}\n", reg.content))]);
                self.edit(buf, &deltas)?;
                self.move_cursor(view, Direction::Down, 1);
                Ok(())
            }
        }
    }

    /// Applies the text object to the pending operator if there is one.
    /// Conceptually this function is quite simple, but there are lot of quirks to match neovim.
    /// If there a question about why it is this way, the answer is probably "because neovim does it".
    pub(crate) fn text_object(
        &mut self,
        selector: impl Selector<ViewId>,
        obj: impl TextObject,
    ) -> Result<(), EditError> {
        let n = self.take_count().unwrap_or(1);
        let obj = obj.repeat(n);
        let (view, buf) = self.get(selector);

        // text objects only have meaning in operator pending mode
        let State::OperatorPending(state) = &self.state else { return Ok(()) };

        let &OperatorPendingState { operator } = state;

        let mut obj_kind = obj.default_kind();
        let flags = obj.flags();

        let text = self.db.buffers[buf].text();

        let cursor = self[view].cursor();
        let target_col = self[view].target_col();
        let Some(mut range) = obj.byte_range(text, text.point_to_byte(cursor)) else {
            // Set cursor to update target column
            // See 0793bc3910afb0982a85f2243b36dcfb2472b5d6
            if flags.contains(TextObjectFlags::UPDATE_TARGET_COLUMN) {
                self.set_cursor(view, cursor);
            }

            self.set_mode(Mode::Normal);
            return Ok(());
        };

        let start_char = text.char_at_byte(range.start);

        let start_point = text.byte_to_point(range.start);
        let inclusive = flags.contains(TextObjectFlags::INCLUSIVE) && !range.is_empty();
        let end_point = text.byte_to_point(range.end);
        let end_point = if inclusive {
            let last = text.char_before_byte(range.end).map_or(0, char::len_utf8);
            text.byte_to_point(range.end - last)
        } else {
            end_point
        };
        // -1 to the line count if the end point is at the start of a line because it's exclusive
        let mut line_count = end_point.line() - start_point.line() + 1;

        let end_adjusted = flags.contains(TextObjectFlags::EXCLUSIVE)
            && end_point.byte_col() == 0
            && obj_kind == TextObjectKind::Charwise
            && line_count > 1;

        if end_adjusted {
            line_count -= 1;
            // special case https://github.com/neovim/neovim/blob/2088521263d3bf9cfd23729adb1a7d152eaab104/src/nvim/ops.c#L6073-L6098
            // tldr;
            //  - if the end point is the first column of a line, we stop before the line terminator of the prior line.
            //  - if the cursor is also before the first non-whitespace character of the line, the motion is executed linewise.
            // This implementation is only roughly approximating neovim's behaviour.
            //
            // Using `start_point` instead of `cursor` as specified in neovim docs as nvim
            // updates the cursor before this point.
            if text.inindent(start_point) {
                obj_kind = TextObjectKind::Linewise;
            } else {
                let line_idx = end_point.line() - 1;
                let line_above = text.line(line_idx).expect("must be in-bounds");
                let adjusted_end_byte =
                    text.point_to_byte(Point::new(line_idx, line_above.len_bytes()));
                if adjusted_end_byte > range.start {
                    // avoid making the range empty
                    range.end = adjusted_end_byte;
                }
            }
        }

        // Another neovim special case inherited from vi.
        // https://github.com/neovim/neovim/blob/efb44e0cad294f51e330d57d7590d38de5cec62c/src/nvim/ops.c#L1468-L1484
        if obj_kind == TextObjectKind::Charwise
            && line_count > 1
            && operator == Operator::Delete
            && !flags.contains(TextObjectFlags::INCLUSIVE)
            && text.inindent(end_point)
            && text.line(end_point.line()).unwrap().chars().all(char::is_whitespace)
        {
            obj_kind = TextObjectKind::Linewise;
        }

        // If we're in a special case for linewise motions that are charwise by default,
        // extend the range to include the full start and end lines.
        if obj.default_kind() == TextObjectKind::Charwise && obj_kind == TextObjectKind::Linewise {
            let start_line = start_point.line();
            let start_byte = text.line_to_byte(start_line);
            let end_byte = text.line_to_byte(start_line + line_count);
            range = start_byte..end_byte;
        }

        let (deltas, new_cursor) = match operator {
            Operator::Delete | Operator::Change => {
                let deltas = Deltas::delete(range.clone());
                let cursor = match obj_kind {
                    // linewise deletions move the line but maintain the column
                    TextObjectKind::Linewise => {
                        PointOrByte::Point(start_point.with_byte_col(cursor.byte_col()))
                    }
                    // charwise deletions moves the cursor to the start of the range
                    TextObjectKind::Charwise => PointOrByte::Byte(range.start),
                };
                (deltas, Some(cursor))
            }
            Operator::Yank => {
                let text = text.byte_slice(range.clone()).to_cow();
                if let Err(err) = with_clipboard!(self, |cb| cb.set_text(text.clone())) {
                    set_error!(self, err);
                }
                self.registers.get_or_insert(Registers::UNNAMED).set(obj_kind, text);
                (Deltas::empty(), None)
            }
        };

        match operator {
            // NOTE: `c` snapshots the buffer before the edit, and `d` saves it after
            Operator::Change => {
                self[buf].snapshot_cursor(start_point);
                self[buf].snapshot(SnapshotFlags::empty());
            }
            Operator::Delete if text.is_empty() => {
                self.set_mode(Mode::Normal);
                return Ok(());
            }
            Operator::Yank | Operator::Delete => {}
        }

        self.edit(view, &deltas)?;

        match operator {
            Operator::Change => self.set_mode(Mode::Insert),
            Operator::Delete => {
                match obj_kind {
                    TextObjectKind::Linewise => {
                        let cursor = match start_char {
                            // Special case if the motion started at the newline of the prior line.
                            Some('\n') if !end_adjusted => cursor,
                            _ if end_adjusted => start_point,
                            _ => start_point.with_byte_col(cursor.byte_col()),
                        };
                        self[buf].snapshot_cursor(cursor);
                        self[buf].snapshot(SnapshotFlags::empty());
                    }
                    TextObjectKind::Charwise => self[buf].snapshot(SnapshotFlags::ALLOW_EMPTY),
                }
                self.set_mode(Mode::Normal);
            }
            Operator::Yank => self.set_mode(Mode::Normal),
        }

        if let Some(new_cursor) = new_cursor {
            // some conditions where the cursor column is set to the old value
            // https://github.com/neovim/neovim/blob/efb44e0cad294f51e330d57d7590d38de5cec62c/src/nvim/ops.c#L6348-L6354
            let (new_cursor, flags) = if obj_kind.is_linewise()
                && !end_adjusted
                && matches!(operator, Operator::Delete)
            {
                assert!(
                    matches!(new_cursor, PointOrByte::Point(_)),
                    "expected point cursor for linewise motion"
                );
                self[view].set_target_col(target_col);
                (new_cursor, SetCursorFlags::FROM_TARGET_COLUMN)
            } else {
                // another vim quirk, move the cursor to the first non-whitespace character
                (new_cursor, SetCursorFlags::START_OF_LINE)
            };

            let (view, buf) = self.db.view_and_buffer_mut(view);
            let area = self.tree.view_area(view.id());
            match new_cursor {
                PointOrByte::Point(point) => {
                    view.set_cursor_linewise(mode!(self), area, buf, point, flags)
                }
                PointOrByte::Byte(byte) => {
                    view.set_cursor_bytewise(mode!(self), area, buf, byte, SetCursorFlags::empty())
                }
            };
        }

        match operator {
            Operator::Delete | Operator::Change => {}
            Operator::Yank => self.dispatch(event::DidYankText { buf, range }),
        }

        Ok(())
    }

    pub fn motion(
        &mut self,
        selector: impl Selector<ViewId>,
        motion: impl Motion,
    ) -> Result<Point, EditError> {
        let view = selector.select(self);
        let (view, buf) = self.db.view_and_buffer_mut(view);
        let view_id = view.id();
        match mode!(self) {
            Mode::OperatorPending(_) => {
                self.text_object(view_id, motion)?;
                Ok(self[view_id].cursor())
            }
            Mode::Normal
            | Mode::Insert
            | Mode::Command
            | Mode::Visual
            | Mode::VisualLine
            | Mode::VisualBlock
            | Mode::ReplacePending => {
                self.search_state.hlsearch = false;

                let text = buf.text();
                let area = self.tree.view_area(view.id());
                let motion_flags = motion.motion_flags();

                let mut flags = SetCursorFlags::empty();
                if motion_flags.contains(MotionFlags::NO_FORCE_UPDATE_TARGET) {
                    flags |= SetCursorFlags::NO_FORCE_UPDATE_TARGET;
                }

                if motion_flags.contains(MotionFlags::USE_TARGET_COLUMN) {
                    flags |= SetCursorFlags::USE_TARGET_COLUMN;
                }

                let point = match motion.motion(text, view.cursor().into()) {
                    PointOrByte::Point(point) => {
                        view.set_cursor_linewise(mode!(self), area, buf, point, flags)
                    }
                    PointOrByte::Byte(byte) => {
                        view.set_cursor_bytewise(mode!(self), area, buf, byte, flags)
                    }
                };
                Ok(point)
            }
        }
    }

    pub fn redo(&mut self, selector: impl Selector<BufferId>) -> Result<bool, EditError> {
        self.undoredo(selector, false)
    }

    pub fn undo(&mut self, selector: impl Selector<BufferId>) -> Result<bool, EditError> {
        self.undoredo(selector, true)
    }

    fn undoredo(
        &mut self,
        selector: impl Selector<BufferId>,
        undo: bool,
    ) -> Result<bool, EditError> {
        let buf = selector.select(self);
        let Some(entry) = (if undo { self[buf].undo() } else { self[buf].redo() }) else {
            return Ok(false);
        };

        if undo {
            for change in entry.changes.iter().rev() {
                self.edit_flags(
                    buf,
                    &change.inversions,
                    EditFlags::NO_RECORD | EditFlags::NO_ENSURE_TRAILING_NEWLINE,
                )?;
            }
        } else {
            for change in &entry.changes[..] {
                self.edit_flags(
                    buf,
                    &change.deltas,
                    EditFlags::NO_RECORD | EditFlags::NO_ENSURE_TRAILING_NEWLINE,
                )?;
            }
        }

        let cursor = match (entry.cursor, entry.changes.first()) {
            (Some(cursor), _) => cursor.into(),
            (_, Some(fst)) => match fst.deltas.iter().next() {
                Some(delta) => delta.range().start.into(),
                None => return Ok(false),
            },
            _ => return Ok(false),
        };

        for view in self.views_into_buf(buf) {
            let area = self.tree.view_area(view);
            let (view, buf) = self.db.view_and_buffer_mut(view);
            match cursor {
                PointOrByte::Point(point) => {
                    view.set_cursor_linewise(mode!(self), area, buf, point, SetCursorFlags::empty())
                }
                PointOrByte::Byte(byte) => {
                    view.set_cursor_bytewise(mode!(self), area, buf, byte, SetCursorFlags::empty())
                }
            };
        }

        Ok(true)
    }

    // Don't think we want this to be a public api, used for tests for now
    #[doc(hidden)]
    pub fn clear_undo(&mut self, selector: impl Selector<BufferId>) {
        self.buffer_mut(selector).clear_undo();
    }

    fn wipe_if_hidden(&mut self, buf: BufferId) {
        if *self[buf].settings().buf_hidden.read() == buffer::BufHidden::Wipe
            && self.db.views.by_buf(&buf).next().is_none()
        {
            self[buf].on_wipe();
            rel::Database::delete(&mut self.db, buf).expect("no view shows it");
        }
    }

    // Manual `impl Future` as we don't want to capture the `'self`
    pub fn save(
        &mut self,
        selector: impl Selector<BufferId>,
        save_flags: SaveFlags,
    ) -> impl Future<Output = crate::Result<()>> + Send + 'static {
        let buf = selector.select(self);

        let buffer = &self[buf];
        let readonly = *buffer.settings().readonly.read();
        let flags = buffer.flags();
        let url = buffer.url().clone();
        let path = buffer.file_path();
        self[buf].snapshot(SnapshotFlags::empty());

        let client = self.client();
        async move {
            if readonly {
                assert!(!flags.contains(BufferFlags::DIRTY), "readonly buffer should not be dirty");
                return Err(
                    io::Error::new(io::ErrorKind::PermissionDenied, "buffer is readonly").into()
                );
            }

            let Some(path) = path else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("buffer `{url}` is not backed by a file"),
                )
                .into());
            };

            let span = tracing::info_span!("save", ?path);
            let _guard = span.enter();

            event::dispatch_async(&client, event::WillSaveBuffer { buf }).await?;

            // Need to refetch flags as the hooks may have updated them
            let (flags, text) = client
                .with(move |editor| {
                    let buf = editor.db.buffers.get(buf)?;
                    Some((buf.flags(), dyn_clone::clone_box(buf.text())))
                })
                .await
                .ok_or_else(|| anyhow!("buffer `{url}` was removed before it was saved"))?;

            if !flags.contains(BufferFlags::DIRTY) && !save_flags.contains(SaveFlags::FORCE) {
                tracing::debug!("buffer is not dirty, skipping write");
                return Ok(());
            }

            file_io::write_buffer(&path, &*text).await?;

            tracing::debug!("buffer written to disk");

            client
                .with(move |editor| {
                    if let Some(b) = editor.db.buffers.get_mut(buf) {
                        b.buffer.flushed();
                        editor.dispatch(event::DidSaveBuffer { buf });
                    }
                })
                .await;
            Ok(())
        }
    }

    pub fn close_view(&mut self, selector: impl Selector<ViewId>) {
        let view = selector.select(self);
        if self.tree.close_view(view).is_err() {
            // already closed
            return;
        }

        self.dispatch(event::DidCloseView { view });

        let closed = &self.db.views[view];
        let (buf, group) = (closed.buffer(), closed.group());
        rel::Database::delete(&mut self.db, view).expect("a view in the tree is in the db");
        self.wipe_if_hidden(buf);

        if let Some(group) = group {
            let views = self.db.views.by_group(&Some(group)).map(|v| v.id()).collect::<Vec<_>>();
            // Ungroups the views, so closing them below doesn't come back here.
            rel::Database::delete(&mut self.db, group).expect("the view's group exists");

            // close all views in the same group
            for v in views {
                self.close_view(v);
            }
        }
    }

    pub fn scroll(&mut self, selector: impl Selector<ViewId>, direction: Direction, amt: usize) {
        let view = selector.select(self);
        let (view, buf) = self.db.view_and_buffer_mut(view);
        let area = self.tree.view_area(view.id());
        view.scroll(mode!(self), area, buf, direction, amt);
    }

    // Not sure if this should be public API
    pub(crate) fn reveal(
        &mut self,
        selector: impl Selector<ViewId>,
        point: impl Into<PointOrByte>,

        alignment: VerticalAlignment,
    ) {
        self.set_cursor(&selector, point);
        self.align_view(&selector, alignment);
    }

    pub fn inspect(&mut self, selector: impl Selector<ViewId>) {
        let inspector_view = self.view(selector).id();
        self.split(inspector_view, Direction::Up, tui::Constraint::Percentage(70));
        let buf = self.db.insert_buffer(|id| Buffer::new(InspectorBuffer::new(id)));
        self.set_buffer(inspector_view, buf);
    }

    /// Create a new view group with the given key.
    /// If a view group with the same key already exists, returns `Err(id)`
    pub fn create_view_group(&mut self, key: impl Into<Ustr>) -> Result<ViewGroupId, ViewGroupId> {
        let key = key.into();
        if let Some(group) = self.db.view_groups.by_key(&key) {
            return Err(group.id());
        }

        Ok(rel::Database::insert_with(&mut self.db, |id| ViewGroup::new(id, key))
            .expect("the key was checked above"))
    }

    // FIXME bad api, not general enough, just used for convenience for now.
    #[doc(hidden)]
    pub fn create_readonly_buffer(
        &mut self,
        path: impl AsRef<Path>,
        s: impl Deref<Target = [u8]> + Send + Sync + 'static,
    ) -> BufferId {
        let theme = self.theme();
        let theme = theme.read();
        self.db.insert_buffer(|id| {
            Buffer::new(TextBuffer::new(
                id,
                buffer::Settings { readonly: Setting::new(true), ..Default::default() },
                filetype!(text),
                path,
                ReadonlyText::new(s),
                &theme,
                None,
            ))
        })
    }

    pub fn create_view(&mut self, buf: BufferId) -> ViewId {
        self.db.insert_view(buf, None, View::new)
    }

    pub fn create_namespace(&mut self, name: impl Into<Ustr>) -> NamespaceId {
        let name = name.into();
        if let Some(ns) = self.db.namespaces.by_name(&name) {
            return ns.id();
        }

        rel::Database::insert_with(&mut self.db, |id| Namespace::new(id, name))
            .expect("the name was checked above")
    }

    pub fn align_view(&mut self, selector: impl Selector<ViewId>, alignment: VerticalAlignment) {
        let view = selector.select(self);
        let area = self.tree.view_area(view);
        self.db.view_mut(view).align(area, alignment);
    }

    pub fn jump_to(&mut self, loc: impl Into<Location>) {
        self.jump(self.current_location(), loc.into());
    }

    fn jump(&mut self, from: Location, to: Location) {
        let jumps = self.view_mut(Active).jump_list_mut();
        jumps.push(from);
        self.goto(to);
    }

    pub(crate) fn goto(&mut self, Location { buf, point }: Location) {
        self.set_buffer(Active, buf);
        self.set_cursor(Active, point);
        self.align_view(Active, VerticalAlignment::Center);
    }

    pub fn search(&mut self, query: &str) -> impl Iterator<Item = &Match> {
        self.search_mode();
        let State::Command(state) = &mut self.state else { unreachable!("search_mode set state") };
        state.buffer.push_str(query);
        self.update_search();
        self.matches()
    }

    pub(crate) fn search_mode(&mut self) {
        self.set_mode(Mode::Command);
        let State::Command(state) = &mut self.state else { unreachable!() };
        state.buffer.clear();
        state.buffer.push('/');
    }

    pub fn jump_forward(&mut self, selector: impl Selector<ViewId>) -> Option<Location> {
        let view = selector.select(self);
        let loc = loop {
            let loc = *self.view_mut(view).jump_list_mut().next()?;
            if self.has_buffer(loc.buf) {
                break loc;
            }
        };
        self.goto(loc);
        Some(loc)
    }

    pub fn jump_back(&mut self, selector: impl Selector<ViewId>) -> Option<Location> {
        let view = selector.select(self);
        let current = self.current_location();
        let loc = loop {
            let loc = *self.view_mut(view).jump_list_mut().prev(current)?;
            if self.has_buffer(loc.buf) {
                break loc;
            }
        };
        self.goto(loc);
        Some(loc)
    }

    fn goto_match(&mut self, f: impl FnOnce(&mut SearchState) -> Option<&Match>) -> Option<Match> {
        self.update_search();

        // reselect the current match if the search is not active
        let mat = if self.search_state.hlsearch {
            f(&mut self.search_state)
        } else {
            self.search_state.current_match()
        }?
        .clone();

        self.search_state.hlsearch = true;
        self.reveal(Active, mat.range().start, VerticalAlignment::Center);
        Some(mat)
    }

    pub fn goto_next_match(&mut self) -> Option<Match> {
        self.goto_match(search::SearchState::next_match)
    }

    pub fn goto_prev_match(&mut self) -> Option<Match> {
        self.goto_match(search::SearchState::prev_match)
    }

    // Bit odd for a method with this name to require a mutable reference.
    // Can consider using some interior mutability.
    pub fn matches(&mut self) -> impl ExactSizeIterator<Item = &Match> {
        self.update_search();
        self.search_state.matches().iter()
    }

    pub fn current_location(&self) -> Location {
        let (view, buf) = self.db.view_and_buffer(self.tree.active());
        Location { buf: buf.id(), point: view.cursor() }
    }

    pub(crate) fn spawn(
        &self,
        desc: impl fmt::Display + Send + 'static,
        fut: impl Future<Output = Result<(), Error>> + Send + 'static,
    ) {
        self.callback(desc, fut, |_, ()| Ok(()));
    }

    pub(crate) fn callback<R: Send + 'static>(
        &self,
        desc: impl fmt::Display + Send + 'static,
        fut: impl Future<Output = Result<R, Error>> + Send + 'static,
        f: impl FnOnce(&mut Editor, R) -> Result<(), Error> + Send + 'static,
    ) {
        callback(&self.callbacks_tx, desc, fut, f);
    }

    #[inline]
    pub fn command_buffer(&self) -> Option<&str> {
        match &self.state {
            State::Command(state) => Some(&state.buffer),
            State::Normal(_)
            | State::Insert(_)
            | State::Visual(_)
            | State::VisualLine(_)
            | State::VisualBlock(_)
            | State::OperatorPending(_)
            | State::ReplacePending => None,
        }
    }

    #[inline]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    #[inline]
    pub fn language_config(&self) -> &language::Config {
        &self.language_config
    }

    #[inline]
    pub fn language_config_mut(&mut self) -> &mut language::Config {
        &mut self.language_config
    }
}

pub trait Selector<T> {
    fn select(&self, editor: &Editor) -> T;
}

impl<T, S: Selector<T> + ?Sized> Selector<T> for &S
where
    S: Selector<T>,
{
    fn select(&self, editor: &Editor) -> T {
        (**self).select(editor)
    }
}

#[derive(Clone, Copy)]
pub struct Active;

impl Selector<ViewId> for Active {
    #[inline]
    fn select(&self, editor: &Editor) -> ViewId {
        editor.tree.active()
    }
}

impl Selector<BufferId> for Active {
    #[inline]
    fn select(&self, editor: &Editor) -> BufferId {
        editor.view(Active).buffer()
    }
}

fn callback<R: Send + 'static>(
    tx: &CallbacksSender,
    desc: impl fmt::Display + Send + 'static,
    fut: impl Future<Output = Result<R, Error>> + Send + 'static,
    f: impl FnOnce(&mut Editor, R) -> Result<(), Error> + Send + 'static,
) {
    callback_with_timeout(tx, desc, Duration::from_secs(3), fut, f);
}

fn callback_with_timeout<R: Send + 'static>(
    tx: &CallbacksSender,
    desc: impl fmt::Display + Send + 'static,
    timeout: Duration,
    fut: impl Future<Output = Result<R, Error>> + Send + 'static,
    f: impl FnOnce(&mut Editor, R) -> Result<(), Error> + Send + 'static,
) {
    tx.send(Box::pin(async move {
        let res = tokio::time::timeout(timeout, fut).await.map_err(
            |_: tokio::time::error::Elapsed| anyhow!("{desc} timed out after {timeout:?}"),
        )??;

        Ok(Box::new(move |editor: &mut Editor| f(editor, res))
            as Box<dyn FnOnce(&mut Editor) -> Result<(), Error> + Send>)
    }))
    .expect("send failed");
}
