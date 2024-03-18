pub(crate) mod cursor;

use std::any::Any;
use std::borrow::Cow;
use std::fs::File;
use std::future::Future;
use std::io::BufReader;
use std::ops::{Deref, Index};
use std::path::{Path, PathBuf};
use std::pin::{pin, Pin};
use std::sync::OnceLock;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use std::{fmt, io};

use anyhow::anyhow;
use futures_util::{Stream, StreamExt};
use ropey::Rope;
use rustc_hash::FxHashMap;
use slotmap::SlotMap;
use stdx::path::PathExt;
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, Notify};
use tui::Widget as _;
use zi_lsp::{lsp_types, LanguageServer as _};

use crate::buffer::{
    BufferFlags, Delta, ExplorerBuffer, FilePicker, Injector, Picker, PickerBuffer, ReadonlyText,
    TextBuffer,
};
use crate::command::{self, Command, CommandKind, Handler, Word};
use crate::input::{Event, KeyCode, KeyEvent, KeySequence};
use crate::keymap::{DynKeymap, Keymap, TrieResult};
use crate::layout::Layer;
use crate::lsp::{self, LanguageClient, LanguageServer};
use crate::motion::{self, Motion};
use crate::plugin::Plugins;
use crate::position::Size;
use crate::syntax::Theme;
use crate::view::{HasViewId, ViewGroup, ViewGroupId};
use crate::{
    event, hashmap, language, layout, trie, Buffer, BufferId, Direction, Error, FileType,
    LanguageServerId, Location, Mode, Point, Url, View, ViewId,
};

bitflags::bitflags! {
    pub struct OpenFlags: u32 {
        const NONE = 0;
        const READONLY = 1 << 0;
        const SPAWN_LANGUAGE_SERVERS = 1 << 1;
        const SET_ACTIVE_BUFFER = 1 << 2;
    }
}

pub struct Editor {
    // pub(crate) to allow `active!` macro to access it
    pub(crate) buffers: SlotMap<BufferId, Box<dyn Buffer + Send>>,
    pub(crate) views: SlotMap<ViewId, View>,
    pub(crate) view_groups: SlotMap<ViewGroupId, ViewGroup>,
    mode: Mode,
    keymap: Keymap,
    theme: Theme,
    language_servers: FxHashMap<LanguageServerId, LanguageServer>,
    callbacks_tx: CallbacksSender,
    requests_tx: tokio::sync::mpsc::Sender<Request>,
    language_config: language::Config,
    tree: layout::ViewTree,
    pool: rayon::ThreadPool,
    /// error to be displayed in the status line
    status_error: Option<String>,
    /// Stores the command currently in the command line
    command: String,
    command_handlers: FxHashMap<Word, Handler>,
    plugins: Plugins,
}

impl Index<ViewId> for Editor {
    type Output = View;

    #[inline]
    fn index(&self, index: ViewId) -> &Self::Output {
        &self.views[index]
    }
}

impl Index<BufferId> for Editor {
    type Output = dyn Buffer;

    #[inline]
    fn index(&self, index: BufferId) -> &Self::Output {
        &self.buffers[index]
    }
}

impl Index<ViewGroupId> for Editor {
    type Output = ViewGroup;

    #[inline]
    fn index(&self, index: ViewGroupId) -> &Self::Output {
        &self.view_groups[index]
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
        #[allow(unused_imports)]
        use $crate::view::HasViewId as _;
        let view_id = $view.view_id();
        let view = &mut $editor.views[view_id];
        let buf = $editor.buffers[view.buffer()].as_any_mut().downcast_mut::<$ty>().expect("buffer downcast failed");
        (view, buf)
    }};
    ($editor:ident: $view:ident) => {{
        #[allow(unused_imports)]
        use $crate::view::HasViewId as _;
        let view_id = $view.view_id();
        let view = &mut $editor.views[view_id];
        let buf = &mut $editor.buffers[view.buffer()];
        (view, buf)
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
        use $crate::view::HasViewId as _;
        let view = &$editor.views[$view.view_id()];
        let buf = &$editor.buffers[view.buffer()];
        (view, buf)
    }};
}

pub(crate) use {get, get_ref};

use self::cursor::SetCursorFlags;

pub type EditorCallback = Box<dyn FnOnce(&mut Editor) -> Result<(), Error>>;

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
        let buf = buffers.insert_with_key(|id| {
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
        let active_view = views.insert_with_key(|id| View::new(id, buf));

        // Using an unbounded channel as we need `callbacks_tx.send()` to be sync.
        let (callbacks_tx, callbacks_rx) = tokio::sync::mpsc::unbounded_channel();

        let (requests_tx, requests_rx) = tokio::sync::mpsc::channel(128);
        let plugins = Plugins::new(Client { tx: requests_tx.clone() });

        let pool = rayon::ThreadPoolBuilder::new().build().expect("rayon pool");
        let mut editor = Self {
            buffers,
            views,
            callbacks_tx,
            requests_tx,
            plugins,
            pool,
            keymap: default_keymap(),
            command_handlers: command::builtin_handlers(),
            tree: layout::ViewTree::new(size, active_view),
            view_groups: Default::default(),
            language_config: Default::default(),
            language_servers: Default::default(),
            mode: Default::default(),
            theme: Default::default(),
            status_error: Default::default(),
            command: Default::default(),
        };

        let notify_redraw = NOTIFY_REDRAW.get_or_init(Default::default);
        editor.resize(size);
        (
            editor,
            Tasks {
                requests: ChannelStream(requests_rx),
                callbacks: UnboundedChannelStream(callbacks_rx),
                notify_redraw,
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
            path = path.canonicalize()?.into();
        }

        let lang = FileType::detect(&path);
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
                let rope = if path.exists() {
                    Rope::from_reader(BufReader::new(File::open(&path)?))?
                } else {
                    Rope::new()
                };

                let buf = TextBuffer::new(
                    buf.id(),
                    BufferFlags::empty(),
                    lang.clone(),
                    &path,
                    rope,
                    &self.theme,
                )
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
                    TextBuffer::new(
                        id,
                        BufferFlags::READONLY,
                        lang.clone(),
                        &path,
                        text,
                        &self.theme,
                    )
                    .boxed()
                } else {
                    let rope = if path.exists() {
                        Rope::from_reader(BufReader::new(File::open(&path)?))?
                    } else {
                        Rope::new()
                    };
                    TextBuffer::new(
                        id,
                        BufferFlags::empty(),
                        lang.clone(),
                        &path,
                        rope,
                        &self.theme,
                    )
                    .boxed()
                };

                tracing::info!(?path, %lang, time = ?start.elapsed(), "opened buffer");
                Ok(buf)
            })?
        };

        if open_flags.contains(OpenFlags::SET_ACTIVE_BUFFER) {
            self.set_active_buffer(buf);
        }

        if open_flags.contains(OpenFlags::SPAWN_LANGUAGE_SERVERS) {
            self.spawn_language_servers_for_lang(buf, &lang)?;
        }

        Ok(buf)
    }

    pub fn register_command(&mut self, handler: Handler) {
        self.command_handlers.insert(handler.name(), handler);
    }

    pub fn open_active(&mut self, path: impl AsRef<Path>) -> Result<BufferId, zi_lsp::Error> {
        self.open(path, OpenFlags::SPAWN_LANGUAGE_SERVERS | OpenFlags::SET_ACTIVE_BUFFER)
    }

    fn set_active_buffer(&mut self, buf: BufferId) {
        self.set_buffer(self.tree.active(), buf);
    }

    pub(crate) fn set_buffer(&mut self, view: ViewId, buf: BufferId) {
        self.views[view].set_buffer(buf);
    }

    pub async fn cleanup(&mut self) {
        for server in std::mem::take(&mut self.language_servers).into_values() {
            // TODO shutdown concurrently
            // async-lsp panics if we terminate too early
            // let _ = tokio::time::timeout(Duration::from_millis(200), server.shutdown()).await;
            let _ = server.shutdown().await;
        }
    }

    pub fn should_quit(&self) -> bool {
        self.tree.is_empty()
    }

    pub fn get_error(&mut self) -> Option<&str> {
        self.status_error.as_deref()
    }

    pub fn set_error(&mut self, error: impl fmt::Display) {
        // TODO push all the corresponding tracing error in here
        set_error!(self, error);
    }

    pub fn render(&mut self, frame: &mut tui::Frame<'_>) {
        let area = self.tree.area();
        let sender = self.sender();

        // Only iterate over the views that are in the view tree, as otherwise they are definitely
        // not visible and we don't need to render them.
        for view in self.tree.views() {
            let view = &self.views[view];
            let buf = &mut self.buffers[view.buffer()];
            let area = self.tree.view_area(view.id());
            buf.pre_render(&sender, view, area);
        }

        self.tree.render(self, frame.buffer_mut());

        // HACK probably there is a nicer way to not special case the cmd and statusline
        let (view, buf) = get_ref!(self);
        let mut status_spans = vec![tui::Span::styled(
            format!(
                "{}:{}:{} ",
                buf.path().display(),
                view.cursor().line() + 1_usize,
                view.cursor().col()
            ),
            tui::Style::new()
                .fg(tui::Color::Rgb(0x88, 0x88, 0x88))
                .bg(tui::Color::Rgb(0x07, 0x36, 0x42)),
        )];

        // The error should probably go in the cmd line not the status line.
        if let Some(error) = &self.status_error {
            status_spans.push(tui::Span::styled(
                error,
                tui::Style::new()
                    .fg(tui::Color::Rgb(0xff, 0x00, 0x00))
                    .bg(tui::Color::Rgb(0x07, 0x36, 0x42)),
            ));
        }

        // FIXME probably a better way than manually padding the right
        status_spans.push(tui::Span::styled(
            " ".repeat(area.width as usize),
            tui::Style::new()
                .fg(tui::Color::Rgb(0x88, 0x88, 0x88))
                .bg(tui::Color::Rgb(0x07, 0x36, 0x42)),
        ));

        let status = tui::Line::default().spans(status_spans);

        let cmd = tui::Text::styled(
            match self.mode {
                Mode::Command => format!(":{}", self.command),
                mode => format!("-- {mode} --",),
            },
            tui::Style::new()
                .fg(tui::Color::Rgb(0x88, 0x88, 0x88))
                .bg(tui::Color::Rgb(0x00, 0x2b, 0x36)),
        );

        let widget = tui::vstack([tui::Constraint::Max(1), tui::Constraint::Max(1)], (status, cmd));

        widget.render(
            tui::Rect { x: 0, y: area.height, width: area.width, height: Self::BOTTOM_BAR_HEIGHT },
            frame.buffer_mut(),
        );

        let (x, y) = self.cursor_viewport_coords();
        let offset = match self.mode {
            Mode::Command => self.command.len() as u16,
            _ => view.line_number_width() as u16,
        };

        frame.set_cursor(x + offset, y);
    }

    pub fn cursor_viewport_coords(&self) -> (u16, u16) {
        if self.mode == Mode::Command {
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
    pub async fn test_run(self, tasks: Tasks) -> io::Result<()> {
        self.run(futures_util::stream::empty(), tasks, |_| Ok(())).await
    }

    pub(crate) fn plugins(&self) -> Plugins {
        self.plugins.clone()
    }

    pub async fn run(
        mut self,
        mut events: impl Stream<Item = io::Result<Event>>,
        Tasks { requests, callbacks, notify_redraw }: Tasks,
        mut render: impl FnMut(&mut Self) -> io::Result<()>,
    ) -> io::Result<()> {
        let plugin_handle = tokio::spawn(self.plugins.clone().run());

        render(&mut self)?;

        let mut requests = requests.fuse();
        let mut callbacks = callbacks.buffer_unordered(16);

        let mut events = pin!(events);
        loop {
            tokio::select! {
                biased;
                Some(event) = events.next() => self.handle_input(event?),
                () = notify_redraw.notified() => tracing::info!("redrawing due to request"),
                req = requests.select_next_some() => {
                    // If the receiver dropped then we just ignore the request.
                    let _ = req.tx.send((req.f)(&mut self));
                },
                f = callbacks.select_next_some() => match f {
                    Ok(f) => if let Err(err) = f(&mut self) {
                        tracing::error!("task callback failed: {err:?}");
                        self.set_error(err);
                    }
                    Err(err) => {
                        tracing::error!("task failed: {err}");
                        self.set_error(err);
                    }
                },
            }

            if self.should_quit() {
                // The only time we abort the plugin system is when we're exiting the main loop so
                // the assumptions above hold.
                plugin_handle.abort();
                let _ = plugin_handle.await;
                break;
            }

            render(&mut self)?;
        }

        self.cleanup().await;

        Ok(())
    }

    #[inline]
    fn handle_key_event(&mut self, key: KeyEvent) {
        self.status_error = None;

        let mut empty = Keymap::default();
        let (_, buf) = get!(self);
        let mut keymap = self.keymap.pair(buf.keymap().unwrap_or(&mut empty));

        tracing::debug!(?key, "handling key");
        match &key.code {
            &KeyCode::Char(_c) if matches!(self.mode, Mode::Insert | Mode::Command) => {
                let (res, buffered) = keymap.on_key(self.mode, key);
                match res {
                    TrieResult::Found(f) => f(self),
                    TrieResult::Partial | TrieResult::Nothing => (),
                }

                for event in buffered {
                    match event.code {
                        KeyCode::Char(c) => match self.mode {
                            Mode::Insert => self.insert_char(c),
                            Mode::Command => self.command.push(c),
                            _ => unreachable!(),
                        },
                        _ => unreachable!(),
                    }
                }
            }
            _ => {
                if let (TrieResult::Found(f), _) = keymap.on_key(self.mode, key) {
                    f(self);
                }
            }
        }
    }

    #[inline]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn execute(&mut self, cmd: Command) {
        let range = cmd.range();
        match cmd.kind() {
            CommandKind::Generic(cmd, args) => {
                if let Some(handler) = self.command_handlers.get(cmd).cloned() {
                    if let Err(err) = handler.execute(self, range, args) {
                        set_error!(self, err);
                    }
                } else {
                    set_error!(self, format!("unknown command: {cmd}"));
                }
            }
        }
    }

    pub fn execute_command(&mut self) {
        match self.command.parse::<Command>() {
            Ok(cmd) => self.execute(cmd),
            Err(err) => set_error!(self, err),
        };

        self.command.clear();
        self.set_mode(Mode::Normal);
    }

    #[inline]
    pub fn set_mode(&mut self, mode: Mode) {
        self.command.clear();

        if let (Mode::Insert, Mode::Normal) = (self.mode, mode) {
            let (view, buf) = get!(self);
            view.move_cursor(mode, self.tree.view_area(view.id()), buf, Direction::Left, 1);
        }

        self.mode = mode;
    }

    #[inline]
    pub fn active_view(&self) -> &View {
        self.view(self.tree.active())
    }

    #[inline]
    pub fn active_buffer(&self) -> &dyn Buffer {
        self.buffer(self.active_view().buffer())
    }

    #[inline]
    pub fn active_buffer_mut(&mut self) -> &mut dyn Buffer {
        let id = self.active_view().buffer();
        &mut self.buffers[id]
    }

    #[inline]
    pub fn active_view_mut(&mut self) -> &mut View {
        let id = self.tree.active();
        &mut self.views[id]
    }

    #[inline]
    pub fn view(&self, id: impl HasViewId) -> &View {
        self.views.get(id.view_id()).expect("bad view id")
    }

    #[inline]
    pub fn view_mut(&mut self, id: ViewId) -> &mut View {
        self.views.get_mut(id).expect("bad view id")
    }

    #[inline]
    pub fn buffers(&self) -> impl ExactSizeIterator<Item = &(dyn Buffer + Send)> {
        self.buffers.values().map(|b| b.as_ref())
    }

    /// An iterator over all views in the view tree.
    // Note: this is not the same as `self.views.values()`
    #[inline]
    pub fn views(&self) -> impl Iterator<Item = &View> {
        self.tree.views().map(move |id| self.view(id))
    }

    #[inline]
    pub fn buffer(&self, id: BufferId) -> &dyn Buffer {
        self.buffers.get(id).expect("got bad buffer id?")
    }

    #[inline]
    pub fn active(&self) -> (&View, &dyn Buffer) {
        let view = self.active_view();
        let buffer = self.buffer(view.buffer());
        (view, buffer)
    }

    #[inline]
    pub fn view_only(&mut self, view: ViewId) {
        self.tree.view_only(view);
    }

    pub fn split_active_view(
        &mut self,
        direction: Direction,
        constraint: tui::Constraint,
    ) -> ViewId {
        let (view, _) = get_ref!(self);
        let id = view.id();
        let view = view.clone();
        let split_view = self.views.insert_with_key(|id| View::split_from(id, view));
        self.tree.split(id, split_view, direction, constraint);
        split_view
    }

    pub fn focus_view(&mut self, id: ViewId) {
        self.tree.focus(id);
    }

    pub fn move_focus(&mut self, direction: Direction) -> ViewId {
        self.tree.move_focus(direction)
    }

    pub fn delete_char_backward(&mut self) {
        let (view, buf) = get!(self);
        if buf.flags().contains(BufferFlags::READONLY) {
            set_error!(self, "buffer is readonly");
            return;
        }

        let cursor = view.cursor();
        let char_idx = buf.text().point_to_char(cursor);
        let start_char_idx = char_idx.saturating_sub(1);
        buf.edit(&Delta::delete(start_char_idx..char_idx)).unwrap();
        let new_cursor = buf.text().char_to_point(start_char_idx);

        view.set_cursor(
            self.mode,
            self.tree.view_area(view.id()),
            buf,
            new_cursor,
            SetCursorFlags::empty(),
        );
    }

    pub fn insert_char(&mut self, c: char) {
        let mut cbuf = [0; 4];
        let view = self.active_view();
        let cursor = view.cursor();
        self.edit(view.id(), &Delta::insert_at(cursor, &*c.encode_utf8(&mut cbuf))).unwrap();

        let (view, buf) = get!(self);
        let area = self.tree.view_area(view.id());
        match c {
            '\n' => view.move_cursor(self.mode, area, buf, Direction::Down, 1),
            _ => view.move_cursor(self.mode, area, buf, Direction::Right, 1),
        };

        let event = event::DidChangeBuffer { buf: buf.id() };
        self.dispatch(event);
    }

    pub fn edit(&mut self, view_id: ViewId, delta: &Delta<'_>) -> Result<(), ropey::Error> {
        // Don't care if we're actually in insert mode, that's more a key binding namespace.
        let (view, buf) = get!(self: view_id);

        if buf.flags().contains(BufferFlags::READONLY) {
            set_error!(self, "buffer is readonly");
            return Ok(());
        }

        let cursor = view.cursor();
        buf.edit(delta)?;
        let buf = buf.id();
        // set the cursor again as it may be out of bounds after the edit
        self.set_cursor(view_id, cursor);

        let event = event::DidChangeBuffer { buf };
        self.dispatch(event);

        Ok(())
    }

    fn dispatch(&mut self, event: impl event::Event) {
        event::dispatch(self, event);
    }

    pub fn insert(&mut self, s: &str) {
        for c in s.chars() {
            self.insert_char(c);
        }
    }

    pub fn current_line(&self) -> Cow<'_, str> {
        let (view, buffer) = self.active();
        let cursor = view.cursor();
        let text = buffer.text();
        text.line(cursor.line().idx())
    }

    pub fn current_char(&self) -> char {
        let (view, buffer) = self.active();
        let cursor = view.cursor();
        let text = buffer.text();
        text.line(cursor.line().idx()).chars().nth(cursor.col().idx()).unwrap()
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn motion(&mut self, motion: impl Motion) {
        let (view, buf) = get!(self);
        let area = self.tree.view_area(view.id());
        let pos = motion.motion(buf.text(), view.cursor());
        view.set_cursor(self.mode, area, buf, pos, SetCursorFlags::empty());
    }

    pub(crate) fn go_to_definition(&mut self) {
        for server in self.language_servers.values_mut() {
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
                        position: pos.into(),
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
        lang: &FileType,
    ) -> zi_lsp::Result<()> {
        if let Some(config) = &self.language_config.languages.get(lang) {
            for server_id in config.language_servers.iter().cloned() {
                if self.language_servers.contains_key(&server_id) {
                    // Language server already running
                    continue;
                }

                let server_config = &self.language_config.language_servers[&server_id];
                let command = &server_config.command;
                let args = &server_config.args;
                tracing::debug!(%server_id, ?command, ?args, "language server initialization");
                let mut server = zi_lsp::Server::start(LanguageClient, ".", command, &args[..])?;
                callback(
                    &self.callbacks_tx,
                    "initializing language server",
                    async move {
                        let res = server
                            .initialize(lsp_types::InitializeParams {
                                capabilities: lsp::capabilities(),
                                ..Default::default()
                            })
                            .await?;
                        tracing::debug!("lsp initialized");
                        server.initialized(lsp_types::InitializedParams {})?;

                        Ok((server, res))
                    },
                    move |editor, (server, res)| {
                        assert!(
                            editor
                                .language_servers
                                .insert(
                                    server_id.clone(),
                                    LanguageServer { server, capabilities: res.capabilities },
                                )
                                .is_none(),
                            "inserted duplicate language server"
                        );

                        register_lsp_event_handlers(server_id);

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
        self.buffers[buf].on_leave();
    }

    pub fn close_active_view(&mut self) {
        self.close_view(self.tree.active())
    }

    pub fn close_view(&mut self, view: ViewId) {
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

    pub fn scroll_active_view(&mut self, direction: Direction, amount: u32) {
        let (view, buf) = get!(self);
        let area = self.tree.view_area(view.id());
        view.scroll(self.mode, area, buf, direction, amount);
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
                    |editor, path: stdx::path::Display| {
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
            editor.set_active_buffer(buf);
            editor.set_mode(Mode::Normal);

            // Cannot use parallel iterator as it doesn't sort.
            let walk = ignore::WalkBuilder::new(path)
                .max_depth(Some(1))
                .sort_by_file_name(std::cmp::Ord::cmp)
                .build();

            editor.pool.spawn(move || {
                let _ = injector.push(PathBuf::from("..").display_owned());
                for entry in walk {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(_) => continue,
                    };

                    if let Err(()) = injector.push(entry.into_path().display_owned()) {
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
    pub fn create_readonly_buffer(
        &mut self,
        name: impl AsRef<Path>,
        s: impl Deref<Target = [u8]> + Send + 'static,
    ) -> BufferId {
        self.buffers.insert_with_key(|id| {
            TextBuffer::new(
                id,
                BufferFlags::READONLY,
                FileType::TEXT,
                name,
                ReadonlyText::new(s),
                &self.theme,
            )
            .boxed()
        })
    }

    pub fn create_buffer(
        &mut self,
        mk: impl FnOnce(BufferId) -> Box<dyn Buffer + Send>,
    ) -> BufferId {
        self.buffers.insert_with_key(mk)
    }

    pub fn create_view(&mut self, buf: BufferId) -> ViewId {
        self.views.insert_with_key(|id| View::new(id, buf))
    }

    pub fn push_layer(&mut self, layer: Layer) {
        self.tree.push(layer);
    }

    pub fn open_picker<P>(
        &mut self,
        view_group_url: Url,
        path: impl AsRef<Path>,
        inject: impl FnOnce(&mut Editor, Injector<P::Item>),
    ) -> ViewGroupId
    where
        P: Picker + Send,
    {
        let view_group = match self.create_view_group(view_group_url) {
            Ok(view_group) => view_group,
            Err(id) => return id,
        };

        let mode = self.mode;
        self.mode = Mode::Insert;

        // FIXME we need to automatically select the first item once available
        let preview_buf = self.create_readonly_buffer("preview", &b""[..]);
        let preview = self.views.insert_with_key(|id| {
            View::new(id, preview_buf)
                .with_line_number(tui::LineNumber::None)
                .with_group(view_group)
        });
        self.tree.push(Layer::new_with_area(preview, |area| {
            tui::Layout::vertical(tui::Constraint::from_percentages([50, 50])).areas::<2>(area)[1]
        }));

        let display_view = self.split_active_view(Direction::Left, tui::Constraint::Fill(1));
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

        let search_view = self.split_active_view(Direction::Up, tui::Constraint::Max(1));
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
                event::HandlerResult::Ok
            }
        });

        let mut injector = None;
        let picker_buf = self.buffers.insert_with_key(|id| {
            let (picker, inj) =
                PickerBuffer::new(id, display_view, request_redraw, P::new(preview));
            injector = Some(inj);
            picker.boxed()
        });
        inject(self, injector.unwrap());

        self.set_buffer(search_view, picker_buf);

        view_group
    }

    pub fn open_jump_list(&mut self) -> ViewGroupId {
        #[derive(Debug, Clone, Copy)]
        struct JumpListPicker {
            preview: ViewId,
        }

        #[derive(Clone)]
        struct Jump {
            buf: BufferId,
            path: PathBuf,
            point: Point,
        }

        impl From<Jump> for Location {
            fn from(val: Jump) -> Self {
                Location { buf: val.buf, point: val.point }
            }
        }

        impl fmt::Display for Jump {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", self.path.display(), self.point)
            }
        }

        impl Picker for JumpListPicker {
            type Item = Jump;

            fn new(preview: crate::ViewId) -> Self {
                Self { preview }
            }

            fn config(self) -> nucleo::Config {
                nucleo::Config::DEFAULT
            }

            fn confirm(self, editor: &mut Editor, item: Jump) {
                let path = &item.path;
                assert!(path.is_file(), "directories should not be in the jump list");
                // We can close any of the views, they are all in the same group
                editor.close_view(self.preview);
                if let Err(err) = editor.open_active(path) {
                    editor.set_error(err);
                }

                editor.jump_to(item);
            }

            // TODO preview the point in the jump list
            fn select(self, editor: &mut Editor, jump: Jump) {
                let path = jump.path;
                match editor.open(path, OpenFlags::READONLY) {
                    Ok(buffer) => editor.set_buffer(self.preview, buffer),
                    Err(err) => editor.set_error(err),
                }
            }
        }

        // Save the current view so the jumps we get are from the right view.
        let view = self.active_view().id();
        self.open_picker::<JumpListPicker>(
            Url::parse("view-group://jumps").unwrap(),
            "jumps",
            |editor, injector| {
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
        self.open_picker::<FilePicker<stdx::path::Display>>(
            Url::parse("view-group://files").unwrap(),
            path,
            |editor, injector| {
                let walk = ignore::WalkBuilder::new(path).build_parallel();
                editor.pool.spawn(move || {
                    walk.run(|| {
                        Box::new(|entry| {
                            let entry = match entry {
                                Ok(entry) => match entry.file_type() {
                                    Some(ft) if ft.is_file() => entry,
                                    _ => return ignore::WalkState::Continue,
                                },
                                Err(_) => return ignore::WalkState::Continue,
                            };

                            match injector.push(entry.into_path().display_owned()) {
                                Ok(()) => ignore::WalkState::Continue,
                                Err(()) => ignore::WalkState::Quit,
                            }
                        })
                    })
                });
            },
        )
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
        let jumps = self.active_view_mut().jump_list_mut();
        jumps.push(from);
        self.goto(to);
    }

    fn goto(&mut self, Location { buf, point }: Location) {
        // FIXME what if buffer is gone
        self.set_active_buffer(buf);
        self.set_active_cursor(point);
    }

    pub fn jump_next(&mut self) -> Option<Location> {
        let loc = self.active_view_mut().jump_list_mut().next().copied()?;
        self.goto(loc);
        Some(loc)
    }

    pub fn jump_prev(&mut self) -> Option<Location> {
        let current = self.current_location();
        let loc = self.active_view_mut().jump_list_mut().prev(current).copied()?;
        self.goto(loc);
        Some(loc)
    }

    fn jump_to_location(&mut self, location: &lsp_types::Location) -> Result<(), Error> {
        let path = location
            .uri
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("lsp returned non-file uri: {}", location.uri))?;

        let from = self.current_location();
        let buf_id = self.open_active(path)?;
        self.jump(from, Location::new(buf_id, location.range.start));

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

    fn callback<R: 'static>(
        &self,
        desc: impl fmt::Display + Send + 'static,
        fut: impl Future<Output = Result<R, Error>> + Send + 'static,
        f: impl FnOnce(&mut Editor, R) -> Result<(), Error> + Send + 'static,
    ) {
        callback(&self.callbacks_tx, desc, fut, f);
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

fn register_lsp_event_handlers(server_id: LanguageServerId) {
    // TODO check capabilities
    event::subscribe_with::<event::DidChangeBuffer>({
        let server_id = server_id.clone();
        move |editor, event| {
            tracing::debug!(?event, "buffer did change");
            let buf = &editor.buffers[event.buf];
            if let (Some(server), Some(uri)) =
                (editor.language_servers.get_mut(&server_id), buf.file_url())
            {
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
            event::HandlerResult::Ok
        }
    });

    event::subscribe_with::<event::DidOpenBuffer>(move |editor, event| {
        let buf = &editor.buffers[event.buf];
        if let (Some(server), Some(uri)) =
            (editor.language_servers.get_mut(&server_id), buf.file_url())
        {
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
        event::HandlerResult::Ok
    });
}

impl Editor {
    /// Return a debug representation of the text and cursor in the active view.
    pub fn display_active(&self) -> impl fmt::Debug + '_ {
        let (view, buf) = self.active();

        struct Debug<'a> {
            view: &'a View,
            buf: &'a dyn Buffer,
        }

        impl fmt::Debug for Debug<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let cursor = self.view.cursor();
                let n = self.buf.text().len_lines();
                for (i, line) in self.buf.text().lines().enumerate() {
                    if i == n - 1 && line.trim().is_empty() {
                        // avoid printing the last empty line
                        break;
                    }

                    write!(f, "{}", i + 1)?;
                    if !line.is_empty() {
                        write!(f, " ")?;
                    }
                    for (j, c) in line.chars().enumerate() {
                        if cursor.line().idx() == i && cursor.col().idx() == j {
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

fn callback<R: 'static>(
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
            as Box<dyn FnOnce(&mut Editor) -> Result<(), Error>>)
    }))
    .expect("send failed");
}

fn default_keymap() -> Keymap<Mode, KeyEvent, Action> {
    // Same as `mem::drop` without the lints.
    // Used to avoid needing braces to ignore values.
    fn void<T>(_: T) {}

    const INSERT_MODE: Action = |editor| editor.set_mode(Mode::Insert);
    const COMMAND_MODE: Action = |editor| editor.set_mode(Mode::Command);
    const INSERT_NEWLINE: Action = |editor| editor.insert_char('\n');
    const NORMAL_MODE: Action = |editor| editor.set_mode(Mode::Normal);
    const MOVE_LEFT: Action = |editor| editor.move_active_cursor(Direction::Left, 1);
    const MOVE_RIGHT: Action = |editor| editor.move_active_cursor(Direction::Right, 1);
    const MOVE_UP: Action = |editor| editor.move_active_cursor(Direction::Up, 1);
    const MOVE_DOWN: Action = |editor| editor.move_active_cursor(Direction::Down, 1);
    const GO_TO_DEFINITION: Action = |editor| editor.go_to_definition();
    const OPEN_NEWLINE: Action = |editor| {
        editor.set_mode(Mode::Insert);
        editor.set_active_cursor(editor.active_cursor().with_col(u32::MAX));
        editor.insert_char('\n');
    };
    const NEXT_TOKEN: Action = |editor| editor.motion(motion::NextToken);
    const PREV_TOKEN: Action = |editor| editor.motion(motion::PrevToken);
    const NEXT_WORD: Action = |editor| editor.motion(motion::NextWord);
    // const PREV_WORD: Action = |editor| editor.motion(motion::PrevWord);
    const APPEND_EOL: Action = |editor| {
        editor.set_active_cursor(editor.active_cursor().with_col(u32::MAX));
        editor.set_mode(Mode::Insert);
        editor.move_active_cursor(Direction::Right, 1);
    };
    const APPEND: Action = |editor| {
        editor.set_mode(Mode::Insert);
        editor.move_active_cursor(Direction::Right, 1);
    };

    const SCROLL_LINE_DOWN: Action = |editor| editor.scroll_active_view(Direction::Down, 1);
    const SCROLL_LINE_UP: Action = |editor| editor.scroll_active_view(Direction::Up, 1);
    const SCROLL_DOWN: Action = |editor| editor.scroll_active_view(Direction::Down, 20);
    const SCROLL_UP: Action = |editor| editor.scroll_active_view(Direction::Up, 20);
    const OPEN_FILE_PICKER: Action = |editor| void(editor.open_file_picker("."));
    const OPEN_JUMP_LIST: Action = |editor| void(editor.open_jump_list());
    const OPEN_FILE_EXPLORER: Action = |editor| editor.open_file_explorer(".");
    const SPLIT_VERTICAL: Action =
        |editor| void(editor.split_active_view(Direction::Right, tui::Constraint::Fill(1)));
    const SPLIT_HORIZONTAL: Action =
        |editor| void(editor.split_active_view(Direction::Down, tui::Constraint::Fill(1)));
    const FOCUS_LEFT: Action = |editor| void(editor.move_focus(Direction::Left));
    const FOCUS_RIGHT: Action = |editor| void(editor.move_focus(Direction::Right));
    const FOCUS_UP: Action = |editor| void(editor.move_focus(Direction::Up));
    const FOCUS_DOWN: Action = |editor| void(editor.move_focus(Direction::Down));
    const VIEW_ONLY: Action = |editor| editor.view_only(editor.active_view().id());
    const EXECUTE_COMMAND: Action = |editor| editor.execute_command();
    const DELETE_CHAR_BACKWARD: Action = |editor| editor.delete_char_backward();
    const JUMP_PREV: Action = |editor| void(editor.jump_prev());
    const JUMP_NEXT: Action = |editor| void(editor.jump_next());

    Keymap::from(hashmap! {
        Mode::Command => trie!({
            "<ESC>" | "<C-c>" => NORMAL_MODE,
            "<CR>" => EXECUTE_COMMAND,
        }),
        Mode::Insert => trie!({
            "<ESC>" | "<C-c>" => NORMAL_MODE,
            "<CR>" => INSERT_NEWLINE,
            "<BS>" => DELETE_CHAR_BACKWARD,
            "f" => {
                "d" => NORMAL_MODE,
            },
        }),
        Mode::Normal => trie!({
            "<C-o>" => JUMP_PREV,
            "<C-i>" => JUMP_NEXT,
            "<C-d>" => SCROLL_DOWN,
            "<C-u>" => SCROLL_UP,
            "<C-e>" => SCROLL_LINE_DOWN,
            "<C-y>" => SCROLL_LINE_UP,
            ":" => COMMAND_MODE,
            "i" => INSERT_MODE,
            "h" => MOVE_LEFT,
            "l" => MOVE_RIGHT,
            "j" => MOVE_DOWN,
            "k" => MOVE_UP,
            "o" => OPEN_NEWLINE,
            "w" => NEXT_WORD,
            // "b" => PREV_WORD,
            "W" => NEXT_TOKEN,
            "B" => PREV_TOKEN,
            "a" => APPEND,
            "A" => APPEND_EOL,
            "<C-h>" => FOCUS_LEFT,
            "<C-j>" => FOCUS_DOWN,
            "<C-k>" => FOCUS_UP,
            "<C-l>" => FOCUS_RIGHT,
            "-" => OPEN_FILE_EXPLORER,
            "<space>" => {
                "e" => OPEN_FILE_EXPLORER,
                "o" => OPEN_FILE_PICKER,
                "j" => OPEN_JUMP_LIST,
            },
            "g" => {
                "d" => GO_TO_DEFINITION,
            },
            "<C-w>" => {
                "o" => VIEW_ONLY,
                "v" | "<C-v>" => SPLIT_VERTICAL,
                "s" | "<C-s>" => SPLIT_HORIZONTAL,
                "h" | "<C-h>" => FOCUS_LEFT,
                "k" | "<C-k>" => FOCUS_UP,
                "j" | "<C-j>" => FOCUS_DOWN,
                "l" | "<C-l>" => FOCUS_RIGHT,
            },
        }),
    })
}
