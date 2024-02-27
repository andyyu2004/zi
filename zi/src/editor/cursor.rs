use super::active;
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
        let (view, buf) = active!(self: view);
        view.set_cursor(self.mode, buf, pos.into(), SetCursorFlags::empty());
    }

    #[inline]
    pub fn move_cursor(&mut self, view: impl HasViewId, direction: Direction, amt: u32) {
        let (view, buf) = active!(self: view);
        view.move_cursor(self.mode, buf, direction, amt);
    }

    #[inline]
    pub fn move_active_cursor(&mut self, direction: Direction, amt: u32) {
        self.move_cursor(self.active_view, direction, amt);
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
