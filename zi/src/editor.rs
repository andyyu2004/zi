pub(crate) mod cursor;

use std::fmt;
use std::fs::File;
use std::future::Future;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll};

use futures_core::Stream;
use ropey::{Rope, RopeBuilder, RopeSlice};
use rustc_hash::FxHashMap;
use slotmap::SlotMap;
use stdx::path::PathExt;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Notify;
use tui::Widget as _;
use zi_lsp::{lsp_types, LanguageServer as _};

use crate::buffer::{PickerBuffer, TextBuffer};
use crate::input::{Event, KeyCode, KeyEvent};
use crate::keymap::{Keymap, TrieResult};
use crate::layout::Layer;
use crate::lsp::{self, LanguageClient, LanguageServer};
use crate::motion::{self, Motion};
use crate::position::Size;
use crate::syntax::Theme;
use crate::{
    event, hashmap, language, layout, trie, Buffer, BufferId, Direction, Error, FileType,
    LanguageServerId, Mode, View, ViewId,
};

pub struct Editor {
    mode: Mode,
    keymap: Keymap<Mode, KeyEvent, Action>,
    buffers: SlotMap<BufferId, Box<dyn Buffer>>,
    views: SlotMap<ViewId, View>,
    theme: Theme,
    language_servers: FxHashMap<LanguageServerId, LanguageServer>,
    tx: CallbacksSender,
    language_config: language::Config,
    tree: layout::ViewTree,
    pool: rayon::ThreadPool,
}

pub type Action = fn(&mut Editor);

static NOTIFY_REDRAW: OnceLock<Notify> = OnceLock::new();

fn request_redraw() {
    NOTIFY_REDRAW.get().expect("editor was not initialized").notify_one()
}

/// Get the active view and buffer.
/// This needs to be a macro so rust can figure out the mutable borrows are disjoint
macro_rules! active {
    ($editor:ident) => {
        active!($editor: $editor.tree.active())
    };
    ($editor:ident: $view:expr) => {{
        #[allow(unused_imports)]
        use $crate::view::HasViewId as _;
        let view_id = $view.view_id();
        let view = &mut $editor.views[view_id];
        let buf = &mut $editor.buffers[view.buffer()];
        (view, buf)
    }};
}

macro_rules! active_ref {
    ($editor:ident) => {
        active_ref!($editor: $editor.tree.active())
    };
    ($editor:ident: $view:expr) => {{
        #[allow(unused_imports)]
        use $crate::view::HasViewId as _;
        let view = &$editor.views[$view.view_id()];
        let buf = &$editor.buffers[view.buffer()];
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
    // status line + command line
    pub const BOTTOM_BAR_HEIGHT: u16 = 1 + 1;

    /// Create a new editor with a scratch buffer.
    /// Returns the editor instance and a stream of callbacks.
    /// The callback stream must be polled and the resulting callback executed on the editor.
    /// The `notify` instance is used to signal the main thread to redraw the screen.
    /// It is recommended to implement a debounce mechanism to avoid redrawing too often.
    pub fn new(size: Size) -> (Self, Callbacks, &'static Notify) {
        let theme = Theme::default();
        let mut buffers = SlotMap::default();
        let buf = buffers.insert_with_key(|id| {
            TextBuffer::new(id, FileType::TEXT, "scratch", "", &theme).boxed()
        });
        let mut views = SlotMap::default();
        let active_view = views.insert_with_key(|id| View::new(id, buf));

        // Using an unbounded channel as we need `tx.send()` to be sync.
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut editor = Self {
            buffers,
            views,
            tx,
            pool: rayon::ThreadPoolBuilder::new().build().expect("rayon pool"),
            tree: layout::ViewTree::new(size, active_view),
            keymap: default_keymap(),
            language_config: Default::default(),
            language_servers: Default::default(),
            mode: Default::default(),
            theme: Default::default(),
        };

        let notify_redraw = NOTIFY_REDRAW.get_or_init(Default::default);
        editor.resize(size);
        (editor, ChannelStream(rx), notify_redraw)
    }

    pub fn open(&mut self, path: impl AsRef<Path>) -> Result<BufferId, zi_lsp::Error> {
        let path = path.as_ref();

        // If the buffer is already open, switch to it
        for buf in self.buffers.values() {
            if buf.path() == path {
                self.views[self.tree.active()].set_buffer(buf.id());
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

        let lang = FileType::detect(path);
        tracing::debug!(%lang, ?path, "detected language");
        let buf = self.buffers.insert_with_key(|id| {
            TextBuffer::new(id, lang.clone(), path, rope, &self.theme).boxed()
        });
        self.views[self.tree.active()].set_buffer(buf);

        self.spawn_language_servers_for_lang(buf, &lang)?;

        Ok(buf)
    }

    pub async fn cleanup(&mut self) {
        for server in std::mem::take(&mut self.language_servers).into_values() {
            // TODO shutdown concurrently
            let _ = server.shutdown().await;
        }
    }

    pub fn should_quit(&self) -> bool {
        self.tree.is_empty()
    }

    pub fn render(&mut self, frame: &mut tui::Frame<'_>) {
        let area = self.tree.area();

        for buf in self.buffers.values_mut() {
            buf.pre_render();
        }

        self.tree.render(self, frame.buffer_mut());

        // HACK probably there is a nicer way to not special case the cmd and statusline
        let (view, buf) = active_ref!(self);
        let statusline = tui::Text::styled(
            format!(
                "{}:{}:{}",
                buf.path().display(),
                view.cursor().line() + 1,
                view.cursor().col()
            ),
            tui::Style::new()
                .fg(tui::Color::Rgb(0x88, 0x88, 0x88))
                .bg(tui::Color::Rgb(0x07, 0x36, 0x42)),
        );

        let cmdline = tui::Text::styled(
            format!("-- {} --", self.mode()),
            tui::Style::new()
                .fg(tui::Color::Rgb(0x88, 0x88, 0x88))
                .bg(tui::Color::Rgb(0x00, 0x2b, 0x36)),
        );

        let widget =
            tui::vstack([tui::Constraint::Max(1), tui::Constraint::Max(1)], (statusline, cmdline));

        widget.render(
            tui::Rect { x: 0, y: area.height, width: area.width, height: Self::BOTTOM_BAR_HEIGHT },
            frame.buffer_mut(),
        );

        let (x, y) = self.cursor_viewport_coords();
        // + 1 for a blank space between line number and text
        const LINE_NR_WIDTH: u16 = 4;
        // FIXME this const is duplicated
        frame.set_cursor(LINE_NR_WIDTH + x + 1, y);
    }

    pub fn cursor_viewport_coords(&self) -> (u16, u16) {
        let (view, buf) = active_ref!(self);
        let area = self.tree.view_area(view.id());
        let (x, y) = view.cursor_viewport_coords(buf);
        (x + area.x, y + area.y)
    }

    pub fn handle_input(&mut self, event: impl Into<Event>) {
        match event.into() {
            Event::Key(key) => self.handle_key_event(key),
            Event::Resize(size) => self.resize(size),
        }
    }

    fn resize(&mut self, size: Size) {
        assert!(size.height > Self::BOTTOM_BAR_HEIGHT, "height must be at least 3");
        // Subtract 2 from the height to leave room for the status line and command line.
        let size = Size { height: size.height - Self::BOTTOM_BAR_HEIGHT, ..size };
        self.tree.resize(size);
        request_redraw();
    }

    #[inline]
    fn handle_key_event(&mut self, key: KeyEvent) {
        tracing::debug!(?key, "handling key");
        match &key.code {
            &KeyCode::Char(_c) if self.mode == Mode::Insert => {
                let (res, buffered) = self.keymap.on_key(self.mode, key);
                match res {
                    TrieResult::Found(f) => f(self),
                    TrieResult::Partial | TrieResult::Nothing => (),
                }

                for event in buffered {
                    match event.code {
                        KeyCode::Char(c) => self.insert_char(c),
                        _ => unreachable!(),
                    }
                }
            }
            _ => {
                if let (TrieResult::Found(f), _) = self.keymap.on_key(self.mode, key) {
                    f(self);
                }
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
    pub fn active_view_mut(&mut self) -> &mut View {
        let id = self.tree.active();
        &mut self.views[id]
    }

    #[inline]
    pub fn view(&self, id: ViewId) -> &View {
        self.views.get(id).expect("got bad view id?")
    }

    #[inline]
    pub fn buffers(&self) -> impl ExactSizeIterator<Item = &dyn Buffer> {
        self.buffers.values().map(|b| b.as_ref())
    }

    #[inline]
    pub fn views(&self) -> impl ExactSizeIterator<Item = &View> + fmt::Debug + Clone {
        self.views.values()
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

    pub fn split_active_view(&mut self, direction: Direction) -> ViewId {
        let (view, _) = active_ref!(self);
        let id = view.id();
        let view = view.clone();
        let split_view = self.views.insert_with_key(|id| View::new_from(id, view));
        self.tree.split(id, split_view, direction);
        split_view
    }

    pub fn focus_view(&mut self, id: ViewId) {
        self.tree.focus(id);
    }

    pub fn move_focus(&mut self, direction: Direction) -> ViewId {
        self.tree.move_focus(direction)
    }

    pub fn insert_char(&mut self, c: char) {
        // Don't care if we're actually in insert mode, that's more a key binding namespace.
        let (view, buf) = active!(self);
        let area = self.tree.view_area(view.id());
        let cursor = view.cursor();
        buf.insert_char(cursor, c);
        match c {
            '\n' => view.move_cursor(self.mode, area, buf, Direction::Down, 1),
            _ => view.move_cursor(self.mode, area, buf, Direction::Right, 1),
        }

        let event = event::DidChangeBuffer { buf: buf.id() };
        self.dispatch(event);
    }

    fn dispatch(&mut self, event: impl event::Event) {
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
        let area = self.tree.view_area(view.id());
        let pos = motion.motion(buf.text().slice(..), view.cursor());
        view.set_cursor(self.mode, area, buf, pos, SetCursorFlags::empty());
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

    pub fn close_active_view(&mut self) {
        let id = self.tree.close_active();
        let view = self.views.remove(id).expect("closed view not found");
        self.buffers[view.buffer()].on_leave();
    }

    pub fn scroll_active_view(&mut self, direction: Direction, amount: u32) {
        let (view, buf) = active!(self);
        let area = self.tree.view_area(view.id());
        view.scroll(self.mode, area, buf, direction, amount);
    }

    pub fn open_file_picker(&mut self, path: impl AsRef<Path>) {
        inner(self, path.as_ref());

        fn inner(editor: &mut Editor, path: &Path) {
            let mut injector = None;
            let buf = editor.buffers.insert_with_key(|id| {
                let (picker, inj) = PickerBuffer::new_streamed(
                    id,
                    nucleo::Config::DEFAULT.match_paths(),
                    request_redraw,
                );
                injector = Some(inj);
                picker.boxed()
            });

            let injector = injector.unwrap();
            let view = editor.views.insert_with_key(|id| View::new(id, buf));
            editor.tree.push(Layer::new(view));

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
            })
        }
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

fn register_lsp_event_handlers(server_id: LanguageServerId) {
    // TODO check capabilities
    event::register(event::handler::<event::DidChangeBuffer>({
        let server_id = server_id.clone();
        move |editor, event| {
            tracing::debug!(?event, "buffer did change");
            let buf = &editor.buffers[event.buf];
            if let (Some(server), Some(uri)) =
                (editor.language_servers.get_mut(&server_id), buf.url())
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
        }
    }));

    event::register(event::handler::<event::DidOpenBuffer>(move |editor, event| {
        let buf = &editor.buffers[event.buf];
        if let (Some(server), Some(uri)) = (editor.language_servers.get_mut(&server_id), buf.url())
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
    }));
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

fn default_keymap() -> Keymap<Mode, KeyEvent, Action> {
    // Same as `mem::drop` without the lints.
    // Used to avoid needing braces to ignore values.
    fn void<T>(_: T) {}

    const INSERT_MODE: Action = |editor| editor.set_mode(Mode::Insert);
    const CLOSE_VIEW: Action = |editor| editor.close_active_view();
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
    const OPEN_FILE_PICKER: Action = |editor| editor.open_file_picker(".");
    const SPLIT_VERTICAL: Action = |editor| void(editor.split_active_view(Direction::Right));
    const SPLIT_HORIZONTAL: Action = |editor| void(editor.split_active_view(Direction::Down));
    const FOCUS_LEFT: Action = |editor| void(editor.move_focus(Direction::Left));
    const FOCUS_RIGHT: Action = |editor| void(editor.move_focus(Direction::Right));
    const FOCUS_UP: Action = |editor| void(editor.move_focus(Direction::Up));
    const FOCUS_DOWN: Action = |editor| void(editor.move_focus(Direction::Down));

    Keymap::new(hashmap! {
        Mode::Normal => trie!({
            "<C-d>" => SCROLL_DOWN,
            "<C-u>" => SCROLL_UP,
            "<C-e>" => SCROLL_LINE_DOWN,
            "<C-y>" => SCROLL_LINE_UP,
            "i" => INSERT_MODE,
            "q" => CLOSE_VIEW,
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
            "<space>" => {
                "o" => OPEN_FILE_PICKER,
            },
            "g" => {
                "d" => GO_TO_DEFINITION,
            },
            "<C-w>" => {
                "v" => SPLIT_VERTICAL,
                "s" => SPLIT_HORIZONTAL,
                "<C-v>" => SPLIT_VERTICAL,
                "<C-s>" => SPLIT_HORIZONTAL,
                "h" => FOCUS_LEFT,
                "l" => FOCUS_RIGHT,
                "j" => FOCUS_DOWN,
                "k" => FOCUS_UP,
                "<C-h>" => FOCUS_LEFT,
                "<C-j>" => FOCUS_DOWN,
                "<C-k>" => FOCUS_UP,
                "<C-l>" => FOCUS_RIGHT,

            },
        }).into_trie(),
        Mode::Insert => trie!({
            "<ESC>" => NORMAL_MODE,
            "<CR>" => INSERT_NEWLINE,
            "f" => {
                "d" => NORMAL_MODE,
            },
        }).into_trie(),
    })
}
