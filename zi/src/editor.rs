pub(crate) mod cursor;

use ropey::{Rope, RopeSlice};
use slotmap::SlotMap;

use crate::event::KeyEvent;
use crate::keymap::{Action, Keymap};
use crate::{Buffer, BufferId, Direction, Mode, View, ViewId};

pub struct Editor {
    pub quit: bool, // tmp hack
    mode: Mode,
    keymap: Keymap,
    buffers: SlotMap<BufferId, Buffer>,
    views: SlotMap<ViewId, View>,
    active_view: ViewId,
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

impl Editor {
    pub fn new(content: impl Into<Rope>) -> Self {
        let mut buffers = SlotMap::default();
        let buf = buffers.insert_with_key(|id| Buffer::new(id, content.into()));
        let mut views = SlotMap::default();
        let active_view = views.insert_with_key(|id| View::new(id, buf));

        Self {
            buffers,
            views,
            active_view,
            mode: Default::default(),
            keymap: Default::default(),
            quit: false,
        }
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
        let text = buf.text_mut();
        let idx = text.line_to_char(cursor.line().idx()) + cursor.col().idx();
        text.insert_char(idx, c);
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
}

impl Default for Editor {
    fn default() -> Self {
        Self::new("")
    }
}
