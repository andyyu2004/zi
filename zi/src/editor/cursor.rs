use crate::view::HasViewId;
use crate::{Direction, Editor, Position};

bitflags::bitflags! {
    pub struct SetCursorFlags: u8 {
        const NO_COLUMN_BOUNDS_CHECK = 0b0001;
    }
}

impl Editor {
    #[inline]
    pub fn set_cursor(&mut self, view: impl HasViewId, pos: impl Into<Position>) {
        let view = &mut self.views[view.view_id()];
        let buf = &self.buffers[view.buffer()];

        view.set_cursor(buf, pos.into(), SetCursorFlags::empty());
    }

    #[inline]
    pub fn move_cursor(&mut self, view: impl HasViewId, direction: Direction) {
        let view = &mut self.views[view.view_id()];
        let buf = &self.buffers[view.buffer()];
        view.move_cursor(buf, direction);
    }

    #[inline]
    pub fn move_active_cursor(&mut self, direction: Direction) {
        self.move_cursor(self.active_view, direction);
    }

    #[inline]
    pub fn active_cursor(&self) -> Position {
        self.active_view().cursor()
    }

    #[inline]
    pub fn set_active_cursor(&mut self, cursor: impl Into<Position>) {
        self.set_cursor(self.active_view, cursor);
    }
}
