use slotmap::SlotMap;

use crate::event::KeyEvent;
use crate::keymap::Keymap;
use crate::{Buffer, BufferId, View, ViewId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
}

pub struct Editor {
    pub quit: bool, // tmp hack
    mode: Mode,
    keymap: Keymap,
    buffers: SlotMap<BufferId, Buffer>,
    views: SlotMap<ViewId, View>,
    active_view: ViewId,
}

impl Editor {
    #[inline]
    pub fn on_key(&mut self, key: KeyEvent) {
        if let Some(f) = self.keymap.on_key(self.mode, key) {
            f(self)
        }
    }

    #[inline]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    #[inline]
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    #[inline]
    pub fn active_view(&self) -> &View {
        self.view(self.active_view).expect("active view not found?")
    }

    #[inline]
    pub fn view(&self, id: ViewId) -> Option<&View> {
        self.views.get(id)
    }

    #[inline]
    pub fn buffer(&self, id: BufferId) -> Option<&Buffer> {
        self.buffers.get(id)
    }

    #[inline]
    pub fn active(&self) -> (&View, &Buffer) {
        let view = self.active_view();
        let buffer = self.buffer(view.buffer()).expect("active buffer not found?");
        (view, buffer)
    }
}

impl Default for Editor {
    fn default() -> Self {
        let mut buffers = SlotMap::default();
        let buf = buffers.insert_with_key(|id| Buffer::new(id, "test text".into()));
        let mut views = SlotMap::default();
        let active_view = views
            // FIXME using random area
            .insert_with_key(|id| View::new(id, buf));

        Self {
            buffers,
            views,
            active_view,
            mode: Default::default(),
            keymap: Default::default(),
            quit: false,
        }
    }
}
