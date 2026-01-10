mod completion;

mod config;
pub(crate) mod cursor;
mod default_keymap;
mod diagnostics;
mod errors;
mod events;
mod lsp_requests;
mod marks;
mod pickers;
mod render;
mod search;
mod state;

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::future::Future;
use std::ops::{self, Deref, Index, IndexMut};
use std::path::{Path, PathBuf};
use std::pin::{Pin, pin};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use std::{cmp, fmt, io, mem};

use anyhow::{anyhow, bail};
use arboard::Clipboard;
use futures_util::stream::FuturesUnordered;
use futures_util::{Stream, StreamExt};
use ignore::WalkState;
use slotmap::SlotMap;
use stdx::path::{PathExt, Relative};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::select;
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tokio::sync::{Notify, oneshot};
use ustr::Ustr;
use zi_core::{PointOrByte, PointRange, Size};
use zi_indent::Indent;
use zi_input::{Event, KeyCode, KeyEvent, KeySequence};
use zi_text::{AnyText, Deltas, ReadonlyText, Rope, RopeBuilder, RopeCursor, Text, TextSlice};
use zi_textobject::motion::{self, Motion, MotionFlags};
use zi_textobject::{TextObject, TextObjectFlags, TextObjectKind};

use self::config::Settings;
use self::diagnostics::BufferDiagnostics;
pub use self::errors::EditError;
pub use self::search::Match;
use self::search::SearchState;
use self::state::{OperatorPendingState, State};
use crate::buffer::picker::{BufferPicker, BufferPickerEntry, DynamicHandler, Picker};
use crate::buffer::{
    Buffer, BufferFlags, EditFlags, ExplorerBuffer, IndentSettings, Injector, InspectorBuffer,
    PickerBuffer, SnapshotFlags, TextBuffer,
};
use crate::command::{self, Command, CommandKind, Handler, Word};
use crate::completion::Completion;
use crate::event::EventHandler;
use crate::keymap::{DynKeymap, Keymap, TrieResult};
use crate::language_service::LanguageServiceInstance;
use crate::layout::Layer;
use crate::plugin::PluginManager;
use crate::syntax::{HighlightId, Syntax, Theme};
use crate::view::{SetCursorFlags, ViewGroup};
use crate::{
    BufferId, Direction, Error, FileType, LanguageService, LanguageServiceId, Location, Mode,
    Namespace, NamespaceId, Operator, Point, Result, Setting, Url, VerticalAlignment, View,
    ViewGroupId, ViewId, event, filetype, language, layout,
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

    pub struct SaveFlags: u32 {
        /// Flush the buffer to disk even if it's not dirty
        const FORCE = 1 << 0;
    }
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
    // Making this public to allow for disjoint mutable borrows to avoid pain.
    pub buffers: SlotMap<BufferId, Buffer>,
    pub views: SlotMap<ViewId, View>,
    pub view_groups: SlotMap<ViewGroupId, ViewGroup>,
    pub active_language_services: HashMap<LanguageServiceId, LanguageServiceInstance>,
    pub language_config: language::Config,
    namespaces: SlotMap<NamespaceId, Namespace>,
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
    clipboard: Clipboard,
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
        &self.views[index]
    }
}

impl IndexMut<ViewId> for Editor {
    #[inline]
    fn index_mut(&mut self, index: ViewId) -> &mut Self::Output {
        &mut self.views[index]
    }
}

impl Index<ViewGroupId> for Editor {
    type Output = ViewGroup;

    #[inline]
    fn index(&self, index: ViewGroupId) -> &Self::Output {
        &self.view_groups[index]
    }
}

impl Index<NamespaceId> for Editor {
    type Output = Namespace;

    #[inline]
    fn index(&self, index: NamespaceId) -> &Self::Output {
        &self.namespaces[index]
    }
}

impl Index<BufferId> for Editor {
    type Output = Buffer;

    #[inline]
    fn index(&self, index: BufferId) -> &Self::Output {
        &self.buffers[index]
    }
}

impl IndexMut<BufferId> for Editor {
    #[inline]
    fn index_mut(&mut self, index: BufferId) -> &mut Self::Output {
        &mut self.buffers[index]
    }
}

pub trait Resource {
    type Id;

    const URL_SCHEME: &'static str;

    fn id(&self) -> Self::Id;

    fn url(&self) -> &Url;
}

pub(crate) type Action = fn(&mut Editor);

static NOTIFY_REDRAW: OnceLock<Notify> = OnceLock::new();

fn request_redraw() {
    NOTIFY_REDRAW.get().expect("editor was not initialized").notify_one()
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

/// Get a view and buffer.
/// This needs to be a macro so rust can figure out the mutable borrows are disjoint
macro_rules! get {
    ($editor:ident as $ty:ty) => {{
        let view_id = $editor.tree().active();
        $crate::editor::get!($editor: view_id as $ty)
    }};
    ($editor:ident as $ty:ty) => {{
        let view_id = $editor.tree().active();
        $crate::editor::get!($editor: view_id as $ty)
    }};
    ($editor:ident) => {{
        let view_id = $editor.tree.active();
        $crate::editor::get!($editor: view_id)
    }};
    ($editor:ident: $view:ident as $ty:ty) => {{
        let view = &mut $editor.views[$view];
        let buf = $editor.buffers[view.buffer()].as_any_mut(Internal(())).downcast_mut::<$ty>().expect("buffer downcast failed");
        (view, buf)
    }};
    ($editor:ident: $view:expr) => {{
        let view = &mut $editor.views[$view];
        let buf = &mut $editor.buffers[view.buffer()];
        (view, buf)
    }};
}

macro_rules! active_servers_of {
    ($editor:ident, $selector:expr) => {{
        let buf = $selector.select($editor);
        $editor
            .active_language_services_by_ft
            .get(&$editor.buffers[buf].file_type())
            .map_or(&[][..], |s| &s[..])
            .iter()
    }};
}

use active_servers_of;

macro_rules! get_ref {
    ($editor:ident as $ty:ty) => {{
        let view_id = $editor.tree().active();
        $crate::editor::get_ref!($editor: view_id as $ty)
    }};
    ($editor:ident: $view:ident as $ty:ty) => {{
        #[allow(unused_imports)]
        use $crate::view::HasViewId as _;
        let view_id = $view.view_id();
        let view = &mut $editor.views[view_id];
        let buf = $editor.buffers[view.buffer()].as_any().downcast_ref::<$ty>().expect("buffer downcast failed");
        (view, buf)
    }};
    ($editor:ident) => {
        $crate::editor::get_ref!($editor: $editor.tree.active())
    };
    ($editor:ident: $view:expr) => {{
        #[allow(unused_imports)]
        let view = &$editor.views[$view];
        let buf = &$editor.buffers[view.buffer()];
        (view, buf)
    }};
}

pub(crate) use {get, get_ref};

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
    #[allow(clippy::type_complexity)]
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
        self.with(move |editor| editor.spawn(desc, fut)).await
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
        let mut buffers = SlotMap::default();
        let theme = settings.theme.read();
        let scratch_buffer = buffers.insert_with_key(|id| {
            Buffer::new(TextBuffer::new(
                id,
                BufferFlags::empty(),
                filetype!(text),
                "scratch",
                Rope::new(),
                &theme,
                None,
            ))
        });
        let mut views = SlotMap::default();
        let active_view = views.insert_with_key(|id| View::new(id, scratch_buffer));

        let mut namespaces = SlotMap::default();
        let default_namespace = namespaces.insert_with_key(|id| Namespace::new(id, "default"));

        let empty_buffer = buffers.insert_with_key(|id| {
            Buffer::new(TextBuffer::new(
                id,
                BufferFlags::READONLY,
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
            buffers,
            views,
            namespaces,
            default_namespace,
            callbacks_tx,
            requests_tx,
            // plugins,
            empty_buffer,
            settings,
            clipboard: Clipboard::new().unwrap(),
            backend: Box::new(backend),
            keymap: default_keymap::new(),
            tree: layout::ViewTree::new(size, active_view),
            command_handlers: command::builtin_handlers(),
            diagnostics: Default::default(),
            notify_quit: Default::default(),
            view_groups: Default::default(),
            language_config: Default::default(),
            active_language_services: Default::default(),
            active_language_services_by_ft: Default::default(),
            state: Default::default(),
            search_state: Default::default(),
            status_error: Default::default(),
            plugin_managers: Default::default(),
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

    fn check_open(&self, path: &mut PathBuf, open_flags: OpenFlags) -> io::Result<()> {
        if path.exists() && !path.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
        }

        if !path.exists() && open_flags.contains(OpenFlags::READONLY) {
            return Err(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        }

        if path.exists() {
            // Try ensure that the file does not contains non-utf8 data.
            use std::io::Read;
            let mut buf = [0u8; 1024];
            let n = File::open(path.as_path())?.read(&mut buf)?;
            match content_inspector::inspect(&buf[..n]) {
                content_inspector::ContentType::UTF_8
                | content_inspector::ContentType::UTF_8_BOM => {}
                _ => {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "non-utf8 data"));
                }
            }

            *path = path.canonicalize()?;
        }

        Ok(())
    }

    fn buffer_at_path(&self, path: &Path) -> Option<BufferId> {
        self.buffers.values().find_map(|b| b.file_path().filter(|p| p == path).map(|_| b.id()))
    }

    pub fn open(
        &mut self,
        path: impl AsRef<Path>,
        open_flags: OpenFlags,
    ) -> io::Result<impl Future<Output = Result<BufferId>> + 'static> {
        let theme = self.theme().clone();
        let mut path = path.as_ref().to_path_buf();
        self.check_open(&mut path, open_flags)?;

        let ft = FileType::detect(&path);
        let syntax = self.backend.new_syntax(ft)?;

        let existing_buf = self.buffer_at_path(&path);

        enum Plan {
            Replace(BufferId),
            Insert,
            Existing(BufferId),
        }

        let plan = if let Some(buf) = existing_buf {
            // If the buffer is already open, we can reuse it.
            // There is an exception where the buffer is already open as a readonly buffer
            // and we want to open it as a normal buffer. In that case we drop the old buffer and
            // replace it with a writable one (with the same id). This is safe as we know we're not
            // losing any data due to it being readonly.
            if self[buf].flags().contains(BufferFlags::READONLY)
                && !open_flags.contains(OpenFlags::READONLY)
                || open_flags.contains(OpenFlags::FORCE)
            {
                Plan::Replace(buf)
            } else {
                Plan::Existing(buf)
            }
        } else {
            Plan::Insert
        };

        let client = self.client();
        Ok(async move {
            #[allow(clippy::too_many_arguments)]
            async fn execute<T: Text + Clone + 'static>(
                client: &Client,
                plan: Plan,
                ft: FileType,
                path: &Path,
                text: T,
                theme: Setting<Theme>,
                flags: BufferFlags,
                syntax: Option<Box<dyn Syntax>>,
            ) -> BufferId {
                let path = path.to_path_buf();
                client
                    .with(move |editor| match plan {
                        Plan::Replace(id) => {
                            let buf = Buffer::new(TextBuffer::new(
                                id,
                                flags,
                                ft,
                                &path,
                                text,
                                &theme.read(),
                                syntax,
                            ));
                            editor.buffers[id] = buf;
                            id
                        }
                        Plan::Insert => editor.buffers.insert_with_key(|id| {
                            Buffer::new(TextBuffer::new(
                                id,
                                flags,
                                ft,
                                &path,
                                text,
                                &theme.read(),
                                syntax,
                            ))
                        }),
                        Plan::Existing(_) => unreachable!(),
                    })
                    .await
            }

            let start = Instant::now();
            let buf = if let Plan::Existing(id) = plan {
                id
            } else if open_flags.contains(OpenFlags::READONLY) {
                debug_assert!(path.exists() && path.is_file());
                // Safety: hmm mmap is tricky, maybe we should try advisory lock the file at least
                let text = unsafe { ReadonlyText::open(&path) }?;
                execute(&client, plan, ft, &path, text, theme, BufferFlags::READONLY, syntax).await
            } else {
                let rope = if path.exists() {
                    rope_from_reader(tokio::fs::File::open(&path).await?).await?
                } else {
                    Rope::new()
                };
                execute(&client, plan, ft, &path, rope, theme, BufferFlags::empty(), syntax).await
            };

            client
                .with(move |editor| {
                    if !open_flags.contains(OpenFlags::BACKGROUND) {
                        editor.set_buffer(Active, buf);
                    }

                    if open_flags.contains(OpenFlags::SPAWN_LANGUAGE_SERVICES) {
                        editor.spawn_language_services_for_ft(buf, ft)?;
                    }

                    editor.dispatch(event::DidOpenBuffer { buf });

                    Ok::<_, Error>(())
                })
                .await?;

            tracing::info!(?path, %ft, time = ?start.elapsed(), "opened buffer");
            Ok(buf)
        })
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
        self.views[view].set_buffer(buf);
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

        let (view, buf) = get_ref!(self);
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

        let mut plugin_manager_handles = FuturesUnordered::from_iter(
            self.plugin_managers.values().cloned().map(|m| tokio::spawn(m.start(self.client()))),
        )
        .fuse();

        let mut requests = pin!(requests.fuse().peekable());
        let mut callbacks = pin!(callbacks.buffer_unordered(128).peekable());

        let mut events = pin!(events);
        loop {
            select! {
                biased;
                Some(event) = events.next() => self.handle_input(event?),
                () = notify_redraw.notified() => tracing::debug!("redrawing due to request"),
                f = callbacks.select_next_some() => match f {
                    Ok(f) => if let Err(err) = f(self) {
                        tracing::error!(error = &*err, "task callback failed");
                        self.set_error(err);
                    }
                    Err(err) => {
                        tracing::error!(error = &*err, "task failed");
                        self.set_error(err);
                    }
                },
                req = requests.select_next_some() => {
                    // If the receiver dropped then we just ignore the request.
                    let _ = req.tx.send((req.f)(self));
                },
                Some(res) = plugin_manager_handles.next() => match res {
                        Ok(Ok(())) => (),
                        Ok(Err(err)) => {
                            tracing::error!(error = &*err, "plugin manager failed");
                            self.set_error(err);
                        }
                        Err(err) => {
                            tracing::error!(error = &err as &dyn std::error::Error, "plugin manager died");
                            self.set_error(err);
                        }
                    },
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

    fn update_search(&mut self) {
        let view = self.tree().active();
        let active_buffer = self[view].buffer();

        let State::Command(state) = &mut self.state else { return };

        let (k, query) = state.buffer().split_at(1);
        match k {
            ":" => {}
            "/" => {
                use regex_cursor::Input;
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

                let (view, buf) = get!(self);

                let text = buf.text();
                let input = Input::new(RopeCursor::new(text.byte_slice(..)));

                let start_time = Instant::now();
                self.search_state.set_matches(
                    text.point_to_byte(view.cursor()),
                    regex
                        .find_iter(input)
                        // This is run synchronously, so we add a strict limit to prevent noticable latency.
                        // However, this may mean not all matches are found which needs a solution.
                        .take(1000)
                        .take_while(|_| start_time.elapsed() < Duration::from_millis(20))
                        .map(|m| {
                            let byte_range = m.range().clone();
                            #[cfg(debug_assertions)]
                            text.byte_slice(byte_range.clone());
                            Match { byte_range }
                        })
                        .collect::<Box<_>>(),
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
            _ => unreachable!(),
        }
    }

    #[inline]
    fn handle_key_event(&mut self, key: KeyEvent) {
        self.status_error = None;
        let mode = mode!(self);

        let mut empty = Keymap::default();
        let (_, buf) = get!(self);
        let mut keymap = self.keymap.pair(buf.keymap().unwrap_or(&mut empty));

        tracing::trace!(%key, "handling key");
        match key.code() {
            KeyCode::Char(c) if matches!(mode, Mode::ReplacePending) => {
                self.set_mode(Mode::Insert);
                self.move_cursor(Active, Direction::Right, 1);
                let _ = self.delete_char(Active);
                let _ = self.insert_char(Active, c);
                self.set_mode(Mode::Normal);
            }
            KeyCode::Char(_c) if matches!(mode, Mode::Insert | Mode::Command) => {
                let (res, buffered) = keymap.on_key(mode, key);
                match res {
                    TrieResult::Found(f) => f(self),
                    TrieResult::Partial | TrieResult::Nothing => (),
                }

                for event in buffered {
                    match event.code() {
                        KeyCode::Char(c) => {
                            if let Err(err) = self.handle_insert(c) {
                                set_error!(self, err);
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
            _ => match keymap.on_key(mode, key).0 {
                TrieResult::Found(f) => f(self),
                TrieResult::Partial => (),
                TrieResult::Nothing => {
                    if matches!(mode, Mode::OperatorPending(_) | Mode::ReplacePending) {
                        self.set_mode(Mode::Normal)
                    }
                }
            },
        }
    }

    #[inline]
    pub fn mode(&self) -> Mode {
        mode!(self)
    }

    pub fn execute<C>(&mut self, cmd: C) -> crate::Result<()>
    where
        C: TryInto<Command>,
        Error: From<<C as TryInto<Command>>::Error>,
    {
        let cmd: Command = cmd.try_into()?;
        let range = cmd.range();
        match cmd.kind() {
            CommandKind::Generic(cmd, args) => {
                if let Some(handler) = self.command_handlers.get(cmd) {
                    handler.execute(self, range.cloned(), args.clone())?;
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

        self.dispatch(event::WillChangeMode { from, to });
        self.state = mem::take(&mut self.state).transition(to);
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

                    start = start.left(c.len_utf8());
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
    pub fn view(&self, selector: impl Selector<ViewId>) -> &View {
        self.views.get(selector.select(self)).expect("bad view id")
    }

    #[inline]
    pub(crate) fn view_mut(&mut self, selector: impl Selector<ViewId>) -> &mut View {
        self.views.get_mut(selector.select(self)).expect("bad view id")
    }

    pub fn text(&self, selector: impl Selector<BufferId>) -> &dyn AnyText {
        self.buffer(selector).text()
    }

    #[inline]
    pub fn buffer(&self, selector: impl Selector<BufferId>) -> &Buffer {
        self.buffers.get(selector.select(self)).expect("bad buffer id")
    }

    #[inline]
    pub(crate) fn buffer_mut(&mut self, selector: impl Selector<BufferId>) -> &mut Buffer {
        self.buffers.get_mut(selector.select(self)).expect("bad buffer id")
    }

    #[inline]
    pub fn buffers(&self) -> impl ExactSizeIterator<Item = &Buffer> {
        self.buffers.values()
    }

    pub fn set_view_group(&mut self, selector: impl Selector<ViewId>, group: ViewGroupId) {
        let view = selector.select(self);
        self.views[view].set_group(group);
    }

    /// An iterator over all views in the view tree.
    // Note: this is not the same as `self.views.values()`
    #[inline]
    pub fn views(&self) -> impl Iterator<Item = &View> {
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
        let (view, _) = get_ref!(self: view_id);
        let id = view.id();
        let view = view.clone();
        let split_view = self.views.insert_with_key(|id| View::split_from(id, view));
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
            _ => {
                let view = selector.select(self);
                let (view, buf) = get!(self: view);
                if buf.flags().contains(BufferFlags::READONLY) {
                    return Err(EditError::Readonly);
                }

                let cursor = view.cursor();
                let text = buf.text();
                let byte_idx = text.point_to_byte(cursor);
                let Some(c) = text.byte_slice(..byte_idx).chars().next_back() else {
                    return Ok(());
                };
                let start_byte_idx =
                    byte_idx.checked_sub(c.len_utf8()).expect("just checked there's a char here");

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

        let (view, buf) = get!(self);
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
                        self.apply_completion_delta(delta);
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
                        self.apply_completion_delta(delta);
                    }
                } else {
                    // TODO
                }
                Ok(())
            }
            // TODO
            State::Visual(..)
            | State::Command(..)
            | State::OperatorPending(_)
            | State::ReplacePending => Ok(()),
        }
    }

    fn indent_newline(&mut self, selector: impl Selector<ViewId>) -> Result<(), EditError> {
        let (view, buf) = self.get(selector);
        let text = self[buf].text();
        let cursor = self[view].cursor();
        if cursor.col() > 0 || !text.line(cursor.line()).unwrap().is_empty() {
            // Simple heuristics to determine if we should indent the new line
            return Ok(());
        }

        let tab_width = *self[buf].settings().tab_width.read();
        match zi_indent::indent(zi_indent::Config { tab_width }, text, cursor.line()) {
            Indent::Bytes(bytes) => {
                let start_byte = text.line_to_byte(cursor.line());
                let deltas = Deltas::insert_at(start_byte, " ".repeat(bytes));
                self.edit(buf, &deltas)?;
                self.set_cursor(view, cursor.right(bytes));
                Ok(())
            }
        }
    }

    fn get(&self, selector: impl Selector<ViewId>) -> (ViewId, BufferId) {
        let view = selector.select(self);
        let buf = self[view].buffer();
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
        use zi_text::Delta;

        let buf = selector.select(self);

        // Set the string to something random otherwise only trivial deltas can be generated.
        let deltas = Deltas::insert_at(0, random_string(rng.clone(), 1000));
        self.edit(buf, &deltas).expect("initial random delta was invalid");

        // TODO should include some special characters like newlines and tabs
        fn random_string(mut rng: impl rand::Rng, n: usize) -> String {
            let len = rng.gen_range(0..=n);
            let chars = rng.sample_iter(&rand::distributions::Alphanumeric);
            chars.take(len).map(|u| u as char).collect()
        }

        let len = self[buf].text().len_bytes();

        let n = rng.gen_range(0..=10);
        let mut k = 0;
        let deltas = Deltas::new((0..n).map_while(|_| {
            if k > len {
                return None;
            }

            let start = if k != len { rng.gen_range(k..=len) } else { k };
            let end = if start != len { rng.gen_range(start..=len) } else { start };
            k = end + 1;
            let range = start..end;

            if rng.gen_bool(0.1) {
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

        if self[buf].flags().contains(BufferFlags::READONLY) {
            return Err(EditError::Readonly);
        }

        // Ensure the buffer ends with a newline before performing an insert.
        if self[buf].flags().contains(BufferFlags::ENSURE_TRAILING_NEWLINE)
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
            .filter(move |&view| self[view].buffer() == buf)
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
        let (view, buffer) = get_ref!(self);
        let cursor = view.cursor();
        let text = buffer.text();
        let line = text.line(cursor.line()).unwrap_or_else(|| Box::new(""));
        line.to_string()
    }

    #[doc(hidden)]
    pub fn cursor_char(&self) -> Option<char> {
        let (view, _) = get_ref!(self);
        let cursor = view.cursor();
        let col = cursor.col();
        self.cursor_line().chars().nth(col)
    }

    pub fn theme(&self) -> Setting<Theme> {
        self.settings().theme.clone()
    }

    pub fn paste(&mut self, selector: impl Selector<ViewId>) -> Result<(), EditError> {
        let text = match self.clipboard.get_text() {
            Ok(text) => text,
            Err(err) => {
                set_error!(self, err);
                return Ok(());
            }
        };

        self.insert(selector, &text)
    }

    /// Applies the text object to the pending operator if there is one.
    /// Conceptually this function is quite simple, but there are lot of quirks to match neovim.
    /// If there a question about why it is this way, the answer is probably "because neovim does it".
    pub(crate) fn text_object(
        &mut self,
        selector: impl Selector<ViewId>,
        obj: impl TextObject,
    ) -> Result<(), EditError> {
        let (view, buf) = self.get(selector);

        // text objects only have meaning in operator pending mode
        let State::OperatorPending(state) = &self.state else { return Ok(()) };

        let &OperatorPendingState { operator } = state;

        let mut motion_kind = obj.default_kind();
        let flags = obj.flags();

        let text = self.buffers[buf].text();

        let cursor = self[view].cursor();
        let target_col = self[view].cursor_target_col();
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
        let end_point = text.byte_to_point(range.end);
        // -1 to the line count if the end point is at the start of a line because it's exclusive
        let mut line_count = end_point.line() - start_point.line() + 1;

        let end_adjusted = flags.contains(TextObjectFlags::EXCLUSIVE)
            && end_point.col() == 0
            && motion_kind == TextObjectKind::Charwise
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
                motion_kind = TextObjectKind::Linewise;
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
        if motion_kind == TextObjectKind::Charwise
            && line_count > 1
            && operator == Operator::Delete
            && text.inindent(end_point)
            && text.line(end_point.line()).unwrap().chars().all(char::is_whitespace)
        {
            motion_kind = TextObjectKind::Linewise;
        }

        // If we're in a special case for linewise motions that are charwise by default,
        // extend the range to include the full start and end lines.
        if obj.default_kind() == TextObjectKind::Charwise && motion_kind == TextObjectKind::Linewise
        {
            let start_line = start_point.line();
            let start_byte = text.line_to_byte(start_line);
            let end_byte = text.line_to_byte(start_line + line_count);
            range = start_byte..end_byte;
        }

        let (deltas, new_cursor) = match operator {
            Operator::Delete | Operator::Change => {
                let deltas = Deltas::delete(range.clone());
                let cursor = match motion_kind {
                    // linewise deletions move the line but maintain the column
                    TextObjectKind::Linewise => {
                        PointOrByte::Point(start_point.with_col(cursor.col()))
                    }
                    // charwise deletions moves the cursor to the start of the range
                    TextObjectKind::Charwise => PointOrByte::Byte(range.start),
                };
                (deltas, Some(cursor))
            }
            Operator::Yank => {
                let text = text.byte_slice(range).to_cow();
                if let Err(err) = self.clipboard.set_text(text) {
                    set_error!(self, err);
                }
                (Deltas::empty(), None)
            }
        };

        match operator {
            // NOTE: `c` snapshots the buffer before the edit, and `d` saves it after
            Operator::Change => {
                self[buf].snapshot_cursor(start_point);
                self[buf].snapshot(SnapshotFlags::empty())
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
                match motion_kind {
                    TextObjectKind::Linewise => {
                        let cursor = match start_char {
                            // Special case if the motion started at the newline of the prior line.
                            Some('\n') if !end_adjusted => cursor,
                            _ if end_adjusted => start_point,
                            _ => start_point.with_col(cursor.col()),
                        };
                        self[buf].snapshot_cursor(cursor);
                        self[buf].snapshot(SnapshotFlags::empty());
                    }
                    TextObjectKind::Charwise => self[buf].snapshot(SnapshotFlags::ALLOW_EMPTY),
                }
                self.set_mode(Mode::Normal)
            }
            Operator::Yank => self.set_mode(Mode::Normal),
        }

        if let Some(new_cursor) = new_cursor {
            // some conditions where the cursor column is set to the old value
            // https://github.com/neovim/neovim/blob/efb44e0cad294f51e330d57d7590d38de5cec62c/src/nvim/ops.c#L6348-L6354
            let (new_cursor, flags) = if motion_kind.is_linewise()
                && !end_adjusted
                && matches!(operator, Operator::Delete)
            {
                let cursor = match new_cursor {
                    PointOrByte::Point(point) => PointOrByte::Point(point.with_col(target_col)),
                    PointOrByte::Byte(_) => panic!("expected point cursor for linewise motion"),
                };
                (cursor, SetCursorFlags::empty())
            } else {
                // another vim quirk, move the cursor to the first non-whitespace character
                (new_cursor, SetCursorFlags::START_OF_LINE)
            };

            let (view, buf) = get!(self: view);
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

        Ok(())
    }

    pub fn motion(
        &mut self,
        selector: impl Selector<ViewId>,
        motion: impl Motion,
    ) -> Result<Point, EditError> {
        let view = selector.select(self);
        let (view, buf) = get!(self: view);
        let view_id = view.id();
        match mode!(self) {
            Mode::OperatorPending(_) => {
                self.text_object(view_id, motion)?;
                Ok(self[view_id].cursor())
            }
            _ => {
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
            let (view, buf) = get!(self: view);
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
        self.buffer_mut(selector).clear_undo()
    }

    fn close_buffer(&mut self, buf: BufferId) {
        // can't naively remove the buffer as it might be referenced by multiple views
        self.buffers[buf].on_leave();
    }

    // Manual `impl Future` as we don't want to capture the `'self`
    pub fn save(
        &mut self,
        selector: impl Selector<BufferId>,
        save_flags: SaveFlags,
    ) -> impl Future<Output = crate::Result<()>> + Send + 'static {
        let buf = selector.select(self);

        let buffer = &self[buf];
        let flags = buffer.flags();
        let url = buffer.url().clone();
        let path = buffer.file_path();
        self[buf].snapshot(SnapshotFlags::empty());

        let client = self.client();
        async move {
            if flags.contains(BufferFlags::READONLY) {
                assert!(!flags.contains(BufferFlags::DIRTY), "readonly buffer should not be dirty");
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "buffer is readonly"))?;
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
                    let buf = &editor[buf];
                    (buf.flags(), dyn_clone::clone_box(buf.text()))
                })
                .await;

            if !flags.contains(BufferFlags::DIRTY) && !save_flags.contains(SaveFlags::FORCE) {
                tracing::info!("buffer is not dirty, skipping write");
                return Ok(());
            }

            use tokio_util::compat::FuturesAsyncReadCompatExt;
            let mut file = tokio::fs::File::create(path).await?;
            let mut reader = futures_util::io::AllowStdIo::new(text.reader()).compat();
            let mut writer = tokio::io::BufWriter::new(&mut file);
            tokio::io::copy(&mut reader, &mut writer).await?;
            writer.flush().await?;
            file.flush().await?;

            tracing::info!("buffer written to disk");

            client
                .with(move |editor| {
                    editor[buf].flushed();
                    editor.dispatch(event::DidSaveBuffer { buf });
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

        // TODO work on a scheme to cleanup views and buffers automatically
        self.close_buffer(self.views[view].buffer());

        if let Some(group) = self.views[view].group() {
            self.view_groups.remove(group);

            // close all views in the same group
            let views = self.views().map(|v| (v.id(), v.group())).collect::<Vec<_>>();
            for (v, g) in views {
                if v == view || g != Some(group) {
                    continue;
                }

                self.close_view(v);
            }
        }
    }

    pub fn scroll(&mut self, selector: impl Selector<ViewId>, direction: Direction, amt: usize) {
        let view = selector.select(self);
        let (view, buf) = get!(self: view);
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

    pub(crate) fn inspect(&mut self, selector: impl Selector<ViewId>) {
        let inspector_view = self.view(selector).id();
        self.split(inspector_view, Direction::Up, tui::Constraint::Percentage(70));
        let buf = self.buffers.insert_with_key(|id| Buffer::new(InspectorBuffer::new(id)));
        self.set_buffer(inspector_view, buf);
    }

    /// Create a new view group with the given url.
    /// If a view group with the same url already exists, returns `Err(id)`
    pub fn create_view_group(&mut self, url: Url) -> Result<ViewGroupId, ViewGroupId> {
        assert_eq!(url.scheme(), ViewGroup::URL_SCHEME);

        if let Some(group) = self.view_groups.values().find(|g| g.url() == &url) {
            return Err(group.id());
        }

        Ok(self.view_groups.insert_with_key(|id| ViewGroup::new(id, url)))
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
        self.buffers.insert_with_key(|id| {
            Buffer::new(TextBuffer::new(
                id,
                BufferFlags::READONLY,
                filetype!(text),
                path,
                ReadonlyText::new(s),
                &theme,
                None,
            ))
        })
    }

    pub fn create_view(&mut self, buf: BufferId) -> ViewId {
        self.views.insert_with_key(|id| View::new(id, buf))
    }

    pub fn create_namespace(&mut self, name: impl Into<Ustr>) -> NamespaceId {
        let name = name.into();
        if let Some(ns) = self.namespaces.values().find(|ns| ns.name() == name) {
            return ns.id();
        }

        self.namespaces.insert_with_key(|id| Namespace::new(id, name))
    }

    pub fn align_view(&mut self, selector: impl Selector<ViewId>, alignment: VerticalAlignment) {
        let view = selector.select(self);
        let (view, buf) = get!(self: view);
        let area = self.tree.view_area(view.id());
        view.align(area, buf, alignment)
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
        // FIXME what if buffer is gone
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
        match &mut self.state {
            State::Command(state) => {
                state.buffer.clear();
                state.buffer.push('/');
            }
            _ => unreachable!(),
        }
    }

    pub fn jump_forward(&mut self, selector: impl Selector<ViewId>) -> Option<Location> {
        let loc = self.view_mut(selector).jump_list_mut().next().copied()?;
        self.goto(loc);
        Some(loc)
    }

    pub fn jump_back(&mut self, selector: impl Selector<ViewId>) -> Option<Location> {
        let current = self.current_location();
        let loc = self.view_mut(selector).jump_list_mut().prev(current).copied()?;
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
        self.goto_match(|s| s.next_match())
    }

    pub fn goto_prev_match(&mut self) -> Option<Match> {
        self.goto_match(|s| s.prev_match())
    }

    // Bit odd for a method with this name to require a mutable reference.
    // Can consider using some interior mutability.
    pub fn matches(&mut self) -> impl ExactSizeIterator<Item = &Match> {
        self.update_search();
        self.search_state.matches().iter()
    }

    pub fn current_location(&self) -> Location {
        let (view, buf) = get_ref!(self);
        Location { buf: buf.id(), point: view.cursor() }
    }

    pub(crate) fn spawn(
        &self,
        desc: impl fmt::Display + Send + 'static,
        fut: impl Future<Output = Result<(), Error>> + Send + 'static,
    ) {
        self.callback(desc, fut, |_, ()| Ok(()))
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
            _ => None,
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

async fn rope_from_reader(reader: impl tokio::io::AsyncRead + Unpin) -> io::Result<Rope> {
    let mut reader = tokio::io::BufReader::new(reader);
    let mut builder = RopeBuilder::new();

    // Handle utf-8 byte order mark.
    // Not supporting other encodings for now.
    let buf = reader.fill_buf().await?;
    if let [0xEF, 0xBB, 0xBF, ..] = buf {
        // Skip the BOM before decoding. This means we won't preserve it on save but we don't care.
        reader.consume(3)
    };

    loop {
        let buf = reader.fill_buf().await?;
        if buf.is_empty() {
            break;
        }

        let s = match std::str::from_utf8(buf) {
            Ok(s) => s,
            Err(err) => {
                let n = err.valid_up_to();
                if n == 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, err));
                }
                unsafe { std::str::from_utf8_unchecked(&buf[..n]) }
            }
        };

        builder.append(s);

        let n = s.len();
        reader.consume(n);
    }

    Ok(builder.build())
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
    tx.send(Box::pin(async move {
        const TIMEOUT: Duration = Duration::from_secs(3);
        let res = tokio::time::timeout(TIMEOUT, fut).await.map_err(
            |_: tokio::time::error::Elapsed| anyhow!("{desc} timed out after {TIMEOUT:?}"),
        )??;

        Ok(Box::new(move |editor: &mut Editor| f(editor, res))
            as Box<dyn FnOnce(&mut Editor) -> Result<(), Error> + Send>)
    }))
    .expect("send failed");
}
