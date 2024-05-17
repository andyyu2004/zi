mod config;
pub(crate) mod cursor;
mod default_keymap;
mod render;
mod search;
mod state;

use std::any::Any;
use std::borrow::Cow;
use std::fs::File;
use std::future::Future;
use std::io::BufReader;
use std::ops::{self, Deref, Index, IndexMut};
use std::path::{Path, PathBuf};
use std::pin::{pin, Pin};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use std::{cmp, fmt, io, mem};

use anyhow::anyhow;
use futures_util::{FutureExt, Stream, StreamExt};
use ignore::WalkState;
use rustc_hash::FxHashMap;
use slotmap::SlotMap;
use stdx::path::{PathExt, Relative};
use tokio::io::AsyncWriteExt;
use tokio::select;
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, Notify};
use zi_core::{PointOrByte, PointRange, Size};
use zi_lsp::lsp_types;
use zi_text::{Deltas, ReadonlyText, Rope, RopeBuilder, RopeCursor, Text, TextSlice};
use zi_textobject::motion::{self, Motion, MotionFlags};
use zi_textobject::{TextObject, TextObjectFlags, TextObjectKind};

use self::config::Config;
pub use self::search::Match;
use self::search::SearchState;
use self::state::{OperatorPendingState, State};
use crate::buffer::picker::{DynamicHandler, PathPicker, PathPickerEntry, Picker};
use crate::buffer::{
    Buffer, BufferFlags, ExplorerBuffer, Injector, InspectorBuffer, PickerBuffer, SnapshotFlags,
    TextBuffer, UndoEntry,
};
use crate::command::{self, Command, CommandKind, Handler, Word};
use crate::event::HandlerResult;
use crate::input::{Event, KeyCode, KeyEvent, KeySequence};
use crate::keymap::{DynKeymap, Keymap, TrieResult};
use crate::layout::Layer;
use crate::lsp::{self, Conv, LanguageServer};
use crate::plugin::Plugins;
use crate::private::{Internal, Sealed};
use crate::syntax::{HighlightId, Theme};
use crate::view::{SetCursorFlags, ViewGroup, ViewGroupId};
use crate::{
    event, language, layout, BufferId, Direction, Error, FileType, LanguageServerId, Location,
    Mode, Operator, Point, Url, VerticalAlignment, View, ViewId,
};

bitflags::bitflags! {
    pub struct OpenFlags: u32 {
        const NONE = 0;
        const READONLY = 1 << 0;
        const SPAWN_LANGUAGE_SERVERS = 1 << 1;
        /// Open the buffer in the active view
        const ACTIVE = 1 << 2;
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

pub struct Editor {
    // pub(crate) to allow `active!` macro to access it
    pub(crate) buffers: SlotMap<BufferId, Box<dyn Buffer>>,
    pub(crate) views: SlotMap<ViewId, View>,
    pub(crate) view_groups: SlotMap<ViewGroupId, ViewGroup>,
    empty_buffer: BufferId,
    config: Config,
    search_state: SearchState,
    state: State,
    keymap: Keymap,
    theme: Theme,
    active_language_servers: FxHashMap<LanguageServerId, LanguageServer>,
    active_language_servers_for_ft: FxHashMap<FileType, Vec<LanguageServerId>>,
    callbacks_tx: CallbacksSender,
    requests_tx: tokio::sync::mpsc::Sender<Request>,
    language_config: language::Config,
    tree: layout::ViewTree,
    /// error to be displayed in the status line
    status_error: Option<String>,
    command_handlers: FxHashMap<Word, Handler>,
    plugins: Plugins,
    notify_quit: Notify,
    notify_idle: &'static Notify,
    is_idle: bool,
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

impl Index<BufferId> for Editor {
    type Output = dyn Buffer;

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

pub type Action = fn(&mut Editor);

static NOTIFY_REDRAW: OnceLock<Notify> = OnceLock::new();

fn request_redraw() {
    NOTIFY_REDRAW.get().expect("editor was not initialized").notify_one()
}

macro_rules! set_error {
    ($editor:ident, $error:expr) => {
        $editor.status_error = Some($error.to_string())
    };
}

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
        // cast away the `Send` bound
        let buf = &mut *$editor.buffers[view.buffer()] as &mut dyn $crate::buffer::Buffer;
        (view, buf)
    }};
}

macro_rules! active_servers_of {
    ($editor:ident, $selector:expr) => {{
        let buf = $selector.select($editor);
        $editor
            .active_language_servers_for_ft
            .get(&$editor.buffers[buf].file_type())
            .map_or(&[][..], |s| &s[..])
            .iter()
    }};
}

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

pub type EditorCallback = Box<dyn FnOnce(&mut Editor) -> Result<(), Error> + Send>;

type Callbacks = impl Stream<Item = CallbackFuture> + Unpin;
type Requests = impl Stream<Item = Request> + Unpin;

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
    tx: Sender<Request>,
}

impl Client {
    pub async fn request<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Editor) -> T + Send + 'static,
    ) -> T {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Request { tx, f: Box::new(|editor| Box::new(f(editor))) })
            .await
            .expect("request receiver should be alive");
        *rx.await.expect("server did not send response").downcast().unwrap()
    }
}

pub struct Tasks {
    requests: Requests,
    callbacks: Callbacks,
    notify_redraw: &'static Notify,
    notify_idle: &'static Notify,
}

impl Editor {
    // status line + command line
    pub const BOTTOM_BAR_HEIGHT: u16 = 1 + 1;

    /// Create a new editor with a scratch buffer.
    /// Returns the editor instance and a stream of callbacks.
    /// The callback stream must be polled and the resulting callback executed on the editor.
    /// The `notify` instance is used to signal the main thread to redraw the screen.
    /// It is recommended to implement a debounce mechanism to avoid redrawing too often.
    pub fn new(size: impl Into<Size>) -> (Self, Tasks) {
        let size = size.into();
        let theme = Theme::default();
        let mut buffers = SlotMap::default();
        let scratch_buffer = buffers.insert_with_key(|id| {
            TextBuffer::new(
                id,
                BufferFlags::empty(),
                FileType::TEXT,
                "scratch",
                Rope::new(),
                &theme,
            )
            .boxed()
        });
        let mut views = SlotMap::default();
        let active_view = views.insert_with_key(|id| View::new(id, scratch_buffer));

        let empty_buffer = buffers.insert_with_key(|id| {
            TextBuffer::new(id, BufferFlags::READONLY, FileType::TEXT, "empty", "", &theme).boxed()
        });

        // Using an unbounded channel as we need `callbacks_tx.send()` to be sync.
        let (callbacks_tx, callbacks_rx) = tokio::sync::mpsc::unbounded_channel();

        let (requests_tx, requests_rx) = tokio::sync::mpsc::channel(128);
        let plugins = Plugins::new(Client { tx: requests_tx.clone() });

        static NOTIFY_IDLE: OnceLock<Notify> = OnceLock::new();
        let notify_idle = NOTIFY_IDLE.get_or_init(Default::default);

        let mut editor = Self {
            buffers,
            views,
            callbacks_tx,
            requests_tx,
            plugins,
            empty_buffer,
            notify_idle,
            is_idle: Default::default(),
            keymap: default_keymap::new(),
            command_handlers: command::builtin_handlers(),
            tree: layout::ViewTree::new(size, active_view),
            notify_quit: Default::default(),
            config: Default::default(),
            view_groups: Default::default(),
            language_config: Default::default(),
            active_language_servers: Default::default(),
            active_language_servers_for_ft: Default::default(),
            state: Default::default(),
            search_state: Default::default(),
            theme: Default::default(),
            status_error: Default::default(),
        };

        let notify_redraw = NOTIFY_REDRAW.get_or_init(Default::default);
        editor.resize(size);

        (
            editor,
            Tasks {
                requests: ChannelStream(requests_rx),
                callbacks: UnboundedChannelStream(callbacks_rx),
                notify_redraw,
                notify_idle,
            },
        )
    }

    pub fn client(&self) -> Client {
        Client { tx: self.requests_tx.clone() }
    }

    pub fn open(
        &mut self,
        path: impl AsRef<Path>,
        open_flags: OpenFlags,
    ) -> Result<BufferId, zi_lsp::Error> {
        let mut path = Cow::Borrowed(path.as_ref());
        if path.exists() && !path.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file").into());
        }

        if !path.exists() && open_flags.contains(OpenFlags::READONLY) {
            return Err(io::Error::new(io::ErrorKind::NotFound, "file not found").into());
        }

        if path.exists() {
            // Try ensure that the file does not contains non-utf8 data.
            use std::io::Read;
            let mut buf = [0u8; 1024];
            let n = File::open(&path)?.read(&mut buf)?;
            match content_inspector::inspect(&buf[..n]) {
                content_inspector::ContentType::UTF_8 => {}
                _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "non-utf8 data").into()),
            }

            path = path.canonicalize()?.into();
        }

        let ft = FileType::detect(&path);

        let buf = if let Some(buf) = self.buffers.values().find(|b| b.path() == path) {
            let id = buf.id();
            // If the buffer is already open, we can reuse it.
            // There is an exception where the buffer is already open as a readonly buffer
            // and we want to open it as a normal buffer. In that case we drop the old buffer and
            // replace it with a writable one (with the same id). This is safe as we know we're not
            // losing any data due to it being readonly.
            if buf.flags().contains(BufferFlags::READONLY)
                && !open_flags.contains(OpenFlags::READONLY)
            {
                let rope =
                    if path.exists() { rope_from_reader(File::open(&path)?)? } else { Rope::new() };

                let buf =
                    TextBuffer::new(buf.id(), BufferFlags::empty(), ft, &path, rope, &self.theme)
                        .boxed();
                self.buffers[id] = buf
            }
            id
        } else {
            self.buffers.try_insert_with_key::<_, io::Error>(|id| {
                let start = Instant::now();
                let buf = if open_flags.contains(OpenFlags::READONLY) {
                    debug_assert!(path.exists() && path.is_file());
                    // Safety: hmm mmap is tricky, maybe we should try advisory lock the file at least
                    let text = unsafe { ReadonlyText::open(&path) }?;
                    TextBuffer::new(id, BufferFlags::READONLY, ft, &path, text, &self.theme).boxed()
                } else {
                    let rope = if path.exists() {
                        rope_from_reader(File::open(&path)?)?
                    } else {
                        Rope::new()
                    };
                    TextBuffer::new(id, BufferFlags::empty(), ft, &path, rope, &self.theme).boxed()
                };

                tracing::info!(?path, %ft, time = ?start.elapsed(), "opened buffer");
                Ok(buf)
            })?
        };

        if open_flags.contains(OpenFlags::ACTIVE) {
            self.set_buffer(Active, buf);
        }

        if open_flags.contains(OpenFlags::SPAWN_LANGUAGE_SERVERS) {
            self.spawn_language_servers_for_lang(buf, ft)?;
        }

        Ok(buf)
    }

    pub fn register_command(&mut self, handler: Handler) {
        self.command_handlers.insert(handler.name(), handler);
    }

    pub fn open_active(&mut self, path: impl AsRef<Path>) -> Result<BufferId, zi_lsp::Error> {
        self.open(path, OpenFlags::SPAWN_LANGUAGE_SERVERS | OpenFlags::ACTIVE)
    }

    pub(crate) fn empty_buffer(&self) -> BufferId {
        self.empty_buffer
    }

    pub fn set_buffer(&mut self, view: impl Selector<ViewId>, buf: impl Selector<BufferId>) {
        let view = view.select(self);
        let buf = buf.select(self);
        self.views[view].set_buffer(buf);
    }

    pub fn highlight_id_by_name(&self, name: impl AsRef<str>) -> HighlightId {
        self.theme.id_by_name(name)
    }

    pub async fn cleanup(&mut self) {
        for mut server in mem::take(&mut self.active_language_servers).into_values() {
            // TODO shutdown concurrenly
            let _ = server.shutdown(()).await;
            let _ = server.exit(());
        }
    }

    #[doc(hidden)]
    pub fn check_quit(&self) -> bool {
        if self.tree.is_empty() {
            self.notify_quit.notify_one();
            return true;
        }

        false
    }

    pub fn get_error(&mut self) -> Option<&str> {
        self.status_error.as_deref()
    }

    /// A hack for tests to wait for background tasks to complete.
    #[doc(hidden)]
    pub fn wait_until_idle(&mut self) -> impl Future + 'static {
        let Self { is_idle, notify_idle, .. } = *self;
        async move {
            if !is_idle {
                let mut fut = pin!(notify_idle.notified());
                fut.as_mut().enable();
                fut.await;
            }
        }
    }

    pub fn set_error(&mut self, error: impl fmt::Display) {
        // TODO push all the corresponding tracing error in here
        set_error!(self, error);
    }

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
            Event::Resize(size) => self.resize(size),
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

    #[cfg(test)]
    pub async fn test_run(mut self, tasks: Tasks) -> io::Result<()> {
        self.run(futures_util::stream::empty(), tasks, |_| Ok(())).await
    }

    pub(crate) fn plugins(&self) -> Plugins {
        self.plugins.clone()
    }

    // HACK, run without spawning the plugin system
    // This can be executed with any executor
    #[doc(hidden)]
    pub async fn fuzz(
        &mut self,
        events: impl Stream<Item = io::Result<Event>>,
        Tasks { requests, callbacks, notify_redraw, notify_idle }: Tasks,
        mut render: impl FnMut(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        Self::subscribe_async_hooks().await;

        render(self)?;

        let mut requests = pin!(requests.fuse().peekable());
        let mut callbacks = pin!(callbacks.buffer_unordered(16).peekable());

        let mut events = pin!(events);
        loop {
            select! {
                biased;
                Some(event) = events.next() => self.handle_input(event?),
                () = notify_redraw.notified() => tracing::info!("redrawing due to request"),
                f = callbacks.select_next_some() => match f {
                    Ok(f) => if let Err(err) = f(self) {
                        tracing::error!("task callback failed: {err:?}");
                        self.set_error(err);
                    }
                    Err(err) => {
                        tracing::error!("task failed: {err}");
                        self.set_error(err);
                    }
                },
                req = requests.select_next_some() => {
                    // If the receiver dropped then we just ignore the request.
                    let _ = req.tx.send((req.f)(self));
                },
                // Put the quit case last to ensure we handle all events first
                () = self.notify_quit.notified() => break,
            }

            if callbacks.as_mut().peek().now_or_never().is_none()
                && requests.as_mut().peek().now_or_never().is_none()
            {
                self.is_idle = true;
                notify_idle.notify_waiters();
                notify_idle.notify_one();
            } else {
                self.is_idle = false;
            }

            // Don't immediately break here as we want to finish handling any events first
            if self.check_quit() {
                // TODO if we don't break here some assertions fail
                break;
            }

            render(self)?;
        }

        self.cleanup().await;

        Ok(())
    }

    pub async fn run(
        &mut self,
        events: impl Stream<Item = io::Result<Event>>,
        tasks: Tasks,
        render: impl FnMut(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        let plugin_handle = tokio::spawn(self.plugins.clone().run());
        self.fuzz(events, tasks, render).await?;
        plugin_handle.abort();
        let _ = plugin_handle.await;

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
                use regex_cursor::engines::meta::Regex;
                use regex_cursor::Input;

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

    fn handle_insert(&mut self, c: char) {
        match &mut self.state {
            State::Insert(..) => self.insert_char(Active, c),
            State::Command(state) => {
                state.buffer.push(c);
                self.update_search();
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
        let mut keymap = self.keymap.pair(buf.keymap(Internal(())).unwrap_or(&mut empty));

        tracing::debug!(%key, "handling key");
        match key.code() {
            KeyCode::Char(_c) if matches!(mode, Mode::Insert | Mode::Command) => {
                let (res, buffered) = keymap.on_key(mode, key);
                match res {
                    TrieResult::Found(f) => f(self),
                    TrieResult::Partial | TrieResult::Nothing => (),
                }

                for event in buffered {
                    match event.code() {
                        KeyCode::Char(c) => self.handle_insert(c),
                        _ => unreachable!(),
                    }
                }
            }
            _ => match keymap.on_key(mode, key).0 {
                TrieResult::Found(f) => f(self),
                TrieResult::Partial => (),
                TrieResult::Nothing => {
                    if matches!(mode, Mode::OperatorPending(_)) {
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
        let cmd = cmd.try_into()?;
        let range = cmd.range();
        match cmd.kind() {
            CommandKind::Generic(cmd, args) => {
                if let Some(handler) = self.command_handlers.get(cmd).cloned() {
                    handler.execute(self, range, args)?;
                } else {
                    anyhow::bail!("unknown command: {cmd}")
                }
            }
        }

        Ok(())
    }

    fn execute_buffered_command(&mut self) {
        let State::Command(state) = &mut self.state else { return };

        if state.buffer.starts_with('/') {
            return self.set_mode(Mode::Normal);
        }

        let Some(cmd) = state.buffer.strip_prefix(':') else {
            return set_error!(self, "command must start with `:`");
        };

        match cmd.parse::<Command>() {
            Ok(cmd) => {
                state.buffer.clear();
                if let Err(err) = self.execute(cmd) {
                    set_error!(self, err);
                }
            }
            Err(err) => set_error!(self, err),
        };

        self.set_mode(Mode::Normal);
    }

    fn insert_to_normal(&mut self) {
        let (view, buf) = get!(self);
        let (view, buf) = (view.id(), buf.id());

        {
            // Clear any whitespace at the end of the cursor line when exiting insert mode
            let cursor = self[view].cursor();
            if let Some(range) = self[buf].text().get_line(cursor.line()).and_then(|line| {
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
                self.edit(Active, &Deltas::delete(byte_range));
            }
        }

        self[buf].snapshot(SnapshotFlags::empty());
        // Move cursor left when exiting insert mode
        self.motion(Active, motion::PrevChar);
    }

    #[inline]
    pub fn set_mode(&mut self, mode: Mode) {
        let from = mode!(self);
        if let (Mode::Insert, Mode::Normal) = (from, mode) {
            self.insert_to_normal()
        }

        self.state = mem::take(&mut self.state).transition(mode);
    }

    #[inline]
    pub fn view(&self, selector: impl Selector<ViewId>) -> &View {
        self.views.get(selector.select(self)).expect("bad view id")
    }

    #[inline]
    pub(crate) fn view_mut(&mut self, selector: impl Selector<ViewId>) -> &mut View {
        self.views.get_mut(selector.select(self)).expect("bad view id")
    }

    #[inline]
    pub fn buffer(&self, selector: impl Selector<BufferId>) -> &dyn Buffer {
        self.buffers.get(selector.select(self)).expect("bad buffer id")
    }

    #[inline]
    pub(crate) fn buffer_mut(&mut self, selector: impl Selector<BufferId>) -> &mut dyn Buffer {
        self.buffers.get_mut(selector.select(self)).expect("bad buffer id")
    }

    #[inline]
    pub fn buffers(&self) -> impl ExactSizeIterator<Item = &dyn Buffer> {
        self.buffers.values().map(|b| b.as_ref())
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
    pub fn delete_char(&mut self, selector: impl Selector<ViewId>) {
        match &mut self.state {
            State::Command(state) => {
                state.buffer.pop();
                if state.buffer.is_empty() {
                    self.set_mode(Mode::Normal);
                }
                self.update_search();
            }
            _ => {
                let view = selector.select(self);
                let (view, buf) = get!(self: view);
                if buf.flags().contains(BufferFlags::READONLY) {
                    // fixme we should return a proper error
                    set_error!(self, "buffer is readonly");
                    return;
                }

                let cursor = view.cursor();
                let text = buf.text();
                let byte_idx = text.point_to_byte(cursor);
                let Some(c) = text.byte_slice(..byte_idx).chars().next_back() else { return };
                let start_byte_idx =
                    byte_idx.checked_sub(c.len_utf8()).expect("just checked there's a char here");

                buf.edit(Internal(()), &Deltas::delete(start_byte_idx..byte_idx));

                view.set_cursor_bytewise(
                    mode!(self),
                    self.tree.view_area(view.id()),
                    buf,
                    start_byte_idx,
                    SetCursorFlags::empty(),
                );
            }
        }
    }

    // Bad API used in tests for now
    #[doc(hidden)]
    pub fn insert_char(&mut self, selector: impl Selector<ViewId>, c: char) {
        let mut cbuf = [0; 4];
        let view = self.view(selector);
        let cursor = view.cursor();
        let cursor_byte = self[view.buffer()].text().point_to_byte(cursor);
        self.edit(view.id(), &Deltas::insert_at(cursor_byte, &*c.encode_utf8(&mut cbuf)));

        let (view, buf) = get!(self);
        let area = self.tree.view_area(view.id());
        match c {
            '\n' => view.move_cursor(mode!(self), area, buf, Direction::Down, 1),
            _ => self.motion(Active, motion::NextChar),
        };
    }

    pub fn edit(&mut self, selector: impl Selector<BufferId>, deltas: &Deltas<'_>) {
        let buf = selector.select(self);
        // // Don't care if we're actually in insert mode, that's more a key binding namespace.
        // let (view, buf) = get!(self: view_id);
        let buf = &mut self[buf];

        if buf.flags().contains(BufferFlags::READONLY) {
            set_error!(self, "buffer is readonly");
            return;
        }

        buf.edit(Internal(()), deltas);
        let buf = buf.id();

        // set the cursor again in relevant views as it may be out of bounds after the edit
        let views =
            self.tree.views().filter(|&view| self[view].buffer() == buf).collect::<Vec<_>>();
        for view in views {
            let cursor = self[view].cursor();
            self.set_cursor_flags(view, cursor, SetCursorFlags::NO_FORCE_UPDATE_TARGET);
        }

        self.dispatch(event::DidChangeBuffer { buf });
    }

    fn dispatch(&mut self, event: impl event::Event) {
        event::dispatch(self, event);
    }

    pub fn insert(&mut self, selector: impl Selector<ViewId>, text: &str) {
        let view = selector.select(self);
        for c in text.chars() {
            self.insert_char(view, c);
        }
    }

    // This and `cursor_char` won't make sense with visual mode
    pub fn cursor_line(&self) -> String {
        let (view, buffer) = get_ref!(self);
        let cursor = view.cursor();
        let text = buffer.text();
        let line = text.get_line(cursor.line()).unwrap_or_else(|| Box::new(""));
        line.to_string()
    }

    pub fn cursor_char(&self) -> Option<char> {
        let (view, _) = get_ref!(self);
        let cursor = view.cursor();
        let col = cursor.col();
        self.cursor_line().chars().nth(col)
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Applies the text object to the pending operator if there is one.
    /// Conceptually this function is quite simple, but there are lot of quirks to match neovim.
    /// If there a question about why it is this way, the answer is probably "because neovim does it".
    pub(crate) fn text_object(&mut self, selector: impl Selector<ViewId>, obj: impl TextObject) {
        let view = selector.select(self);
        let (view, buf) = get!(self: view);
        let (view, buf) = (view.id(), buf.id());

        // text objects only have meaning in operator pending mode
        let State::OperatorPending(state) = &self.state else { return };

        let &OperatorPendingState { operator } = state;

        let mut motion_kind = obj.default_kind();
        let flags = obj.flags();

        let text = self[buf].text();

        let cursor = self[view].cursor();
        let target_col = self[view].cursor_target_col();
        let Some(mut range) = obj.byte_range(text, text.point_to_byte(cursor)) else {
            // Set cursor to update target column
            // See 0793bc3910afb0982a85f2243b36dcfb2472b5d6
            if flags.contains(TextObjectFlags::UPDATE_TARGET_COLUMN) {
                self.set_cursor(view, cursor);
            }
            return self.set_mode(Mode::Normal);
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
            if inindent(text, start_point) {
                motion_kind = TextObjectKind::Linewise;
            } else {
                let line_idx = end_point.line() - 1;
                let line_above = text.get_line(line_idx).expect("must be in-bounds");
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
            && inindent(text, end_point)
            && text.get_line(end_point.line()).unwrap().chars().all(char::is_whitespace)
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
            Operator::Yank => todo!(),
        };

        match operator {
            // `c` snapshot the buffer before the edit, and delete saves it after
            Operator::Change => {
                self[buf].snapshot_cursor(start_point);
                self[buf].snapshot(SnapshotFlags::empty())
            }
            Operator::Delete if text.is_empty() => return self.set_mode(Mode::Normal),
            Operator::Yank | Operator::Delete => {}
        }

        self.edit(view, &deltas);

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
            Operator::Yank => {}
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
    }

    pub fn motion(&mut self, selector: impl Selector<ViewId>, motion: impl Motion) -> Point {
        let view = selector.select(self);
        let (view, buf) = get!(self: view);
        let view_id = view.id();
        match mode!(self) {
            Mode::OperatorPending(_) => {
                self.text_object(view_id, motion);
                self[view_id].cursor()
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

                match motion.motion(text, view.cursor().into()) {
                    PointOrByte::Point(point) => {
                        view.set_cursor_linewise(mode!(self), area, buf, point, flags)
                    }
                    PointOrByte::Byte(byte) => {
                        view.set_cursor_bytewise(mode!(self), area, buf, byte, flags)
                    }
                }
            }
        }
    }

    pub fn redo(&mut self, selector: impl Selector<ViewId>) -> bool {
        self.undoredo(selector, |buf| buf.redo())
    }

    pub fn undo(&mut self, selector: impl Selector<ViewId>) -> bool {
        self.undoredo(selector, |buf| buf.undo())
    }

    fn undoredo(
        &mut self,
        selector: impl Selector<ViewId>,
        f: impl FnOnce(&mut dyn Buffer) -> Option<UndoEntry>,
    ) -> bool {
        let view = selector.select(self);
        let (view, buf) = get!(self: view);
        let Some(entry) = f(buf) else { return false };

        let cursor = match (entry.cursor, entry.changes.first()) {
            (Some(cursor), _) => cursor.into(),
            (_, Some(fst)) => fst.deltas.iter().next().unwrap().range().start.into(),
            _ => return false,
        };

        let area = self.tree.view_area(view.id());
        match cursor {
            PointOrByte::Point(point) => {
                view.set_cursor_linewise(mode!(self), area, buf, point, SetCursorFlags::empty())
            }
            PointOrByte::Byte(byte) => {
                view.set_cursor_bytewise(mode!(self), area, buf, byte, SetCursorFlags::empty())
            }
        };

        let buf = buf.id();
        self.dispatch(event::DidChangeBuffer { buf });
        true
    }

    // Don't think we want this to be a public api, used for tests for now
    #[doc(hidden)]
    pub fn clear_undo(&mut self) {
        self.buffer_mut(Active).clear_undo()
    }

    pub(crate) fn goto_definition(&mut self) {
        for server_id in active_servers_of!(self, Active) {
            let server = self.active_language_servers.get_mut(server_id).unwrap();
            match &server.capabilities.definition_provider {
                Some(lsp_types::OneOf::Left(true) | lsp_types::OneOf::Right(_)) => (),
                _ => continue,
            }

            let (view, buf) = get!(self);
            let pos = view.cursor();

            if let Some(uri) = buf.file_url() {
                tracing::debug!(%uri, %pos, "lsp request definition");
                let fut = server.definition(lsp_types::GotoDefinitionParams {
                    text_document_position_params: lsp_types::TextDocumentPositionParams {
                        text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
                        position: pos.conv(),
                    },
                    work_done_progress_params: lsp_types::WorkDoneProgressParams {
                        work_done_token: None,
                    },
                    partial_result_params: lsp_types::PartialResultParams {
                        partial_result_token: None,
                    },
                });

                self.callback(
                    "go to definition request",
                    async move { Ok(fut.await?) },
                    |editor, res| {
                        tracing::debug!(?res, "lsp definition response");
                        editor.jump_to_definition(res)?;
                        Ok(())
                    },
                );
            }

            // Send the request to the first server that supports it
            return;
        }

        self.set_error("no active language server supports go to definition");
    }

    fn spawn_language_servers_for_lang(
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

                let mut server = self.language_config.language_servers[&server_id].spawn()?;

                callback(
                    &self.callbacks_tx,
                    "initializing language server",
                    async move {
                        let res = server
                            .initialize(lsp_types::InitializeParams {
                                capabilities: lsp::client_capabilities(),
                                ..Default::default()
                            })
                            .await?;

                        Ok((res, server))
                    },
                    move |editor, (res, mut server)| {
                        tracing::debug!("lsp initialized");
                        server.initialized(lsp_types::InitializedParams {})?;

                        assert!(
                            editor
                                .active_language_servers
                                .insert(server_id, LanguageServer::new(res.capabilities, server))
                                .is_none(),
                            "inserted duplicate language server"
                        );

                        editor
                            .active_language_servers_for_ft
                            .entry(ft)
                            .or_default()
                            .push(server_id);

                        subscribe_lsp_event_handlers(server_id);

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

    fn close_buffer(&mut self, buf: BufferId) {
        // can't naively remove the buffer as it might be referenced by multiple views
        self.buffers[buf].on_leave(Internal(()));
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
        let path = buffer.path().to_path_buf();

        let client = self.client();
        async move {
            if flags.contains(BufferFlags::READONLY) {
                assert!(!flags.contains(BufferFlags::DIRTY), "readonly buffer should not be dirty");
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "buffer is readonly"))?;
            }

            event::dispatch_async(&client, event::WillSaveBuffer { buf }).await?;

            // Need to refetch flags as the hooks may have updated them
            let (flags, text) = client
                .request(move |editor| {
                    let buf = &editor[buf];
                    (buf.flags(), dyn_clone::clone_box(buf.text()))
                })
                .await;

            if !flags.contains(BufferFlags::DIRTY) && !save_flags.contains(SaveFlags::FORCE) {
                tracing::info!("buffer is not dirty, skipping save");
                return Ok(());
            }

            use tokio_util::compat::FuturesAsyncReadCompatExt;
            let mut file = tokio::fs::File::create(path).await?;
            let mut reader = futures_util::io::AllowStdIo::new(text.reader()).compat();
            let mut writer = tokio::io::BufWriter::new(&mut file);
            tokio::io::copy(&mut reader, &mut writer).await?;
            writer.flush().await?;
            file.flush().await?;

            client.request(move |editor| editor[buf].flushed(Internal(()))).await;
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

    pub(crate) fn inspect(&mut self) {
        let inspector_view = self.view(Active).id();
        self.split(Active, Direction::Up, tui::Constraint::Percentage(70));
        let buf = self.buffers.insert_with_key(|id| InspectorBuffer::new(id).boxed());
        self.set_buffer(inspector_view, buf);
    }

    pub fn open_file_explorer(&mut self, path: impl AsRef<Path>) {
        inner(self, path.as_ref());

        fn inner(editor: &mut Editor, path: &Path) {
            let mut injector = None;
            let buf = editor.buffers.insert_with_key(|id| {
                let (explorer, inj) = ExplorerBuffer::new(
                    id,
                    nucleo::Config::DEFAULT.match_paths(),
                    request_redraw,
                    |editor, path: Relative| {
                        let path = path.into_inner();
                        if path.is_dir() {
                            editor.open_file_explorer(path);
                        } else if let Err(err) = editor.open_active(path) {
                            editor.set_error(err);
                        }
                    },
                );
                injector = Some(inj);
                explorer.boxed()
            });

            let injector = injector.unwrap();
            editor.set_buffer(Active, buf);
            editor.set_mode(Mode::Normal);

            // Cannot use parallel iterator as it doesn't sort.
            let walk = ignore::WalkBuilder::new(path)
                .max_depth(Some(1))
                .sort_by_file_path(|a, b| {
                    if a.is_dir() && !b.is_dir() {
                        cmp::Ordering::Less
                    } else if !a.is_dir() && b.is_dir() {
                        cmp::Ordering::Greater
                    } else {
                        a.cmp(b)
                    }
                })
                .build();

            let path = path.to_path_buf();
            pool().spawn(move || {
                let _ = injector.push(PathBuf::from("..").display_relative_to(&path));
                for entry in walk {
                    let Ok(entry) = entry else { continue };
                    if let Err(()) = injector.push(entry.into_path().display_relative_to(&path)) {
                        break;
                    }
                }
            })
        }
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
        self.buffers.insert_with_key(|id| {
            TextBuffer::new(
                id,
                BufferFlags::READONLY,
                FileType::TEXT,
                path,
                ReadonlyText::new(s),
                &self.theme,
            )
            .boxed()
        })
    }

    pub fn create_buffer(&mut self, mk: impl FnOnce(BufferId) -> Box<dyn Buffer>) -> BufferId {
        self.buffers.insert_with_key(mk)
    }

    pub fn create_view(&mut self, buf: BufferId) -> ViewId {
        self.views.insert_with_key(|id| View::new(id, buf))
    }

    pub fn push_layer(&mut self, layer: Layer) {
        self.tree.push(layer);
    }

    fn open_static_picker<P>(
        &mut self,
        view_group_url: Url,
        path: impl AsRef<Path>,
        f: impl FnOnce(&mut Self, Injector<P::Entry>),
    ) -> ViewGroupId
    where
        P: Picker + Send,
    {
        self.open_picker::<P>(view_group_url, path, None, f)
    }

    fn open_dynamic_picker<P>(
        &mut self,
        view_group_url: Url,
        path: impl AsRef<Path>,
        dynamic_source: impl Fn(Injector<P::Entry>, &str) + Send + Sync + 'static,
    ) -> ViewGroupId
    where
        P: Picker + Send,
    {
        self.open_picker::<P>(view_group_url, path, Some(Arc::new(dynamic_source)), |_, _| {})
    }

    fn open_picker<P>(
        &mut self,
        view_group_url: Url,
        path: impl AsRef<Path>,
        dynamic_source: Option<DynamicHandler<P::Entry>>,
        f: impl FnOnce(&mut Self, Injector<P::Entry>),
    ) -> ViewGroupId
    where
        P: Picker + Send,
    {
        let view_group = match self.create_view_group(view_group_url) {
            Ok(view_group) => view_group,
            Err(id) => return id,
        };

        let mode = mode!(self);
        self.set_mode(Mode::Insert);

        let preview_buf = self.create_readonly_buffer("preview", &b""[..]);
        let preview = self.views.insert_with_key(|id| {
            let view = View::new(id, preview_buf).with_group(view_group);
            view.config().line_number_style.write(tui::LineNumberStyle::None);
            view
        });

        let (a, b) = *self.config.picker_split_proportion.read();
        self.tree.push(Layer::new_with_area(preview, move |area| {
            tui::Layout::vertical(tui::Constraint::from_fills([a, b])).areas::<2>(area)[1]
        }));

        let display_view = self.split(Active, Direction::Left, tui::Constraint::Fill(1));
        self.views[display_view].set_buffer(self.buffers.insert_with_key(|id| {
            TextBuffer::new(
                id,
                BufferFlags::empty(),
                FileType::TEXT,
                path,
                Rope::new(),
                &self.theme,
            )
            .boxed()
        }));

        let search_view = self.split(Active, Direction::Up, tui::Constraint::Max(1));
        assert_eq!(self.tree().active(), search_view);

        // ensure all views are in the same group so they close together
        self.views[display_view].set_group(view_group);
        self.views[search_view].set_group(view_group);

        event::subscribe_with::<event::DidCloseView>({
            move |editor, event| {
                // restore the mode if the picker view group is closed
                if editor.views[event.view].group() == Some(view_group) {
                    editor.set_mode(mode);
                    return event::HandlerResult::Unsubscribe;
                }
                event::HandlerResult::Continue
            }
        });

        let mut injector = None;
        let picker_buf = self.buffers.insert_with_key(|id| {
            let mut picker = PickerBuffer::new(id, display_view, request_redraw, P::new(preview));
            injector = Some(picker.injector());
            if let Some(source) = dynamic_source {
                picker = picker.with_dynamic_handler(source);
            }
            picker.boxed()
        });

        f(self, injector.unwrap());

        self.set_buffer(search_view, picker_buf);

        view_group
    }

    pub fn open_jump_list(&mut self) -> ViewGroupId {
        #[derive(Clone, Debug)]
        struct Jump {
            buf: BufferId,
            path: PathBuf,
            point: Point,
        }

        #[allow(clippy::from_over_into)]
        impl Into<Location> for Jump {
            fn into(self) -> Location {
                Location { buf: self.buf, point: self.point }
            }
        }

        impl fmt::Display for Jump {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", self.path.display(), self.point)
            }
        }

        impl PathPickerEntry for Jump {
            #[inline]
            fn path(&self) -> &Path {
                &self.path
            }

            #[inline]
            fn line(&self) -> Option<usize> {
                Some(self.point.line())
            }
        }

        // Save the current view so the jumps we get are from the right view.
        let view = self.view(Active).id();
        self.open_static_picker::<PathPicker<_>>(
            Url::parse("view-group://jumps").unwrap(),
            "jumps",
            move |editor, injector| {
                for loc in editor.view(view).jump_list().iter() {
                    let path = editor.buffer(loc.buf).path().to_path_buf();
                    if let Err(()) = injector.push(Jump { path, buf: loc.buf, point: loc.point }) {
                        break;
                    }
                }
            },
        )
    }

    pub fn open_file_picker(&mut self, path: impl AsRef<Path>) -> ViewGroupId {
        let path = path.as_ref();
        self.open_static_picker::<PathPicker<stdx::path::Display>>(
            Url::parse("view-group://files").unwrap(),
            path,
            |_editor, injector| {
                let mut entries =
                    ignore::WalkBuilder::new(path).build().filter_map(|entry| match entry {
                        Ok(entry) => match entry.file_type() {
                            Some(ft) if ft.is_file() => Some(entry),
                            _ => None,
                        },
                        Err(err) => {
                            tracing::error!(%err, "file picker error");
                            None
                        }
                    });

                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(50);
                for entry in entries.by_ref() {
                    if let Err(()) = injector.push(entry.into_path().display_owned()) {
                        break;
                    }

                    if std::time::Instant::now() > deadline {
                        pool().spawn(move || {
                            for entry in entries {
                                if let Err(()) = injector.push(entry.into_path().display_owned()) {
                                    break;
                                }
                            }
                        });
                        break;
                    }
                }
            },
        )
    }

    pub fn open_global_search(&mut self, path: impl AsRef<Path>) -> ViewGroupId {
        #[derive(Clone, Debug)]
        struct Entry {
            #[allow(unused)]
            // TODO can be used to highlight the matching portion of the line
            byte_range: ops::Range<usize>,
            path: PathBuf,
            line: usize,
            content: String,
        }

        impl PathPickerEntry for Entry {
            #[inline]
            fn path(&self) -> &Path {
                &self.path
            }

            #[inline]
            fn line(&self) -> Option<usize> {
                Some(self.line)
            }
        }

        impl fmt::Display for Entry {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{} {}", self.path.display(), self.line, self.content)
            }
        }

        let path = path.as_ref().to_path_buf();
        self.open_dynamic_picker::<PathPicker<Entry>>(
            Url::parse("view-group://search").unwrap(),
            "search",
            move |injector, query| {
                tracing::debug!(%query, "global search update");

                let matcher = search::matcher(query);
                let searcher = search::searcher();

                let walk = ignore::WalkBuilder::new(&path).build_parallel();

                pool().spawn(move || {
                    walk.run(|| {
                        let injector = injector.clone();
                        let mut searcher = searcher.clone();
                        let matcher = matcher.clone();

                        Box::new(move |entry| {
                            let entry = match entry {
                                Ok(entry) => match entry.file_type() {
                                    Some(ft) if ft.is_file() => entry,
                                    _ => return WalkState::Continue,
                                },
                                Err(_) => return WalkState::Continue,
                            };

                            let mut quit = false;
                            let sink = search::Sink(|line, content, byte_range| {
                                quit = injector
                                    .push(Entry {
                                        byte_range,
                                        line: line.checked_sub(1).expect("1-indexed") as usize,
                                        path: entry.path().to_path_buf(),
                                        content: content.trim_end().to_string(),
                                    })
                                    .is_err();
                                Ok(!quit)
                            });

                            // TODO search buffers first so unsaved content will show

                            if let Err(err) = searcher.search_path(&matcher, entry.path(), sink) {
                                tracing::error!(%err, "global search error");
                            }

                            if quit {
                                tracing::debug!("global search cancelled");
                                WalkState::Quit
                            } else {
                                WalkState::Continue
                            }
                        })
                    })
                });
            },
        )
    }

    pub fn align_view(&mut self, selector: impl Selector<ViewId>, alignment: VerticalAlignment) {
        let view = selector.select(self);
        let (view, buf) = get!(self: view);
        let area = self.tree.view_area(view.id());
        view.align(area, buf, alignment)
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

    pub fn jump_forward(&mut self) -> Option<Location> {
        let loc = self.view_mut(Active).jump_list_mut().next().copied()?;
        self.goto(loc);
        Some(loc)
    }

    pub fn jump_back(&mut self) -> Option<Location> {
        let current = self.current_location();
        let loc = self.view_mut(Active).jump_list_mut().prev(current).copied()?;
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

    fn jump_to_location(&mut self, location: &lsp_types::Location) -> Result<(), Error> {
        let path = location
            .uri
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("lsp returned non-file uri: {}", location.uri))?;

        let from = self.current_location();
        let buf_id = self.open_active(path)?;
        self.jump(from, Location::new(buf_id, location.range.start.conv()));

        Ok(())
    }

    pub fn current_location(&self) -> Location {
        let (view, buf) = get_ref!(self);
        Location { buf: buf.id(), point: view.cursor() }
    }

    fn sender(&self) -> SyncClient {
        SyncClient(self.callbacks_tx.clone())
    }

    pub(crate) fn schedule(
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
    pub fn config(&self) -> &Config {
        &self.config
    }

    #[inline]
    pub fn language_config(&self) -> &language::Config {
        &self.language_config
    }

    #[inline]
    pub fn language_config_mut(&mut self) -> &mut language::Config {
        &mut self.language_config
    }

    async fn subscribe_async_hooks() {
        event::subscribe_async_with::<event::WillSaveBuffer, _>(|client, event| async move {
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
                            let deltas = lsp::proto::deltas_from_lsp_edits(text, edits);

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
        .await;
    }
}

/// A synchronous client to the editor.
pub struct SyncClient(CallbacksSender);

impl SyncClient {
    pub fn request(&self, f: impl FnOnce(&mut Editor) -> Result<(), Error> + Send + 'static) {
        // no description needed as `ready()` will never timeout
        callback(&self.0, "", std::future::ready(Ok(())), |editor, ()| f(editor));
    }
}

fn subscribe_lsp_event_handlers(server_id: LanguageServerId) {
    // TODO check capabilities
    event::subscribe_with::<event::DidChangeBuffer>({
        move |editor, event| {
            tracing::debug!(?event, "buffer did change");
            let buf = &editor.buffers[event.buf];
            if let (Some(server), Some(uri)) =
                (editor.active_language_servers.get_mut(&server_id), buf.file_url())
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

                tracing::debug!(%uri, ?server_id, "lsp did_change");
                server
                    .did_change(lsp_types::DidChangeTextDocumentParams {
                        text_document: lsp_types::VersionedTextDocumentIdentifier {
                            uri: uri.clone(),
                            version: buf.version() as i32,
                        },
                        content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
                            range: None,
                            range_length: None,
                            text: buf.text().to_string(),
                        }],
                    })
                    .expect("lsp did_change failed");
            }
            event::HandlerResult::Continue
        }
    });

    event::subscribe_with::<event::DidOpenBuffer>(move |editor, event| {
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
    });
}

impl Editor {
    /// Return a debug representation of the text and cursor in the active view.
    pub fn display_view(&self, selector: impl Selector<ViewId>) -> impl fmt::Debug + '_ {
        let (view, buf) = get_ref!(self: selector.select(self));

        struct Debug<'a> {
            view: &'a View,
            buf: &'a dyn Buffer,
        }

        impl fmt::Debug for Debug<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let cursor = self.view.cursor();
                for (i, line) in self.buf.text().byte_slice(..).lines().enumerate() {
                    write!(f, "{}", i + 1)?;
                    if !line.is_empty() {
                        write!(f, " ")?;
                    }

                    for (j, c) in line.chars().chain(Some('\n')).enumerate() {
                        if cursor.line() == i && cursor.col() == j {
                            write!(f, "|")?;
                            if c == '\n' {
                                // The cursor can be on the newline character in insert mode.
                                // We still need to write the newline character.
                                writeln!(f)?;
                            }
                        } else {
                            write!(f, "{c}")?;
                        }
                    }
                }

                Ok(())
            }
        }

        Debug { view, buf }
    }
}

/// Returns true if the cursor is on or before the first non-whitespace character of the line.
fn inindent(text: impl Text, cursor: Point) -> bool {
    text.get_line(cursor.line())
        .expect("cursor should be inbounds")
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| c.len_utf8())
        .sum::<usize>()
        >= cursor.col()
}

#[cfg(test)]
#[test]
fn test_inindent() {
    #[track_caller]
    fn check(text: impl Text, cursor: Point, expect: bool) {
        assert_eq!(inindent(text, cursor), expect);
    }

    check("a", Point::new(0, 0), true);
    check(" a", Point::new(0, 0), true);
    check(" a", Point::new(0, 1), true);
    check(" a", Point::new(0, 2), false);
}

fn rope_from_reader(reader: impl io::Read) -> io::Result<Rope> {
    use std::io::BufRead;

    let mut reader = BufReader::new(reader);
    let mut builder = RopeBuilder::new();

    loop {
        let buf = reader.fill_buf()?;
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

pub trait Selector<T>: Sealed {
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

impl Sealed for Active {}

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
