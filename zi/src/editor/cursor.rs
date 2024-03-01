use super::active;
use crate::view::HasViewId;
use crate::{Direction, Editor, Position};

bitflags::bitflags! {
    // A bunch of hacks, don't make this public
    pub(crate) struct SetCursorFlags: u8 {
        const NO_COLUMN_BOUNDS_CHECK = 1 << 0;
        const MOVE_TO_LAST_LINE_IF_OUT_OF_BOUNDS = 1 << 1;
    }
}

impl Editor {
    #[inline]
    pub fn set_cursor(&mut self, view: impl HasViewId, pos: impl Into<Position>) {
        let (view, buf) = active!(self: view);
        let size = self.tree.size(view.id());
        view.set_cursor(self.mode, size, buf, pos.into(), SetCursorFlags::empty());
    }

    #[inline]
    pub fn move_cursor(&mut self, view: impl HasViewId, direction: Direction, amt: u32) {
        let (view, buf) = active!(self: view);
        let size = self.tree.size(view.id());
        view.move_cursor(self.mode, size, buf, direction, amt);
    }

    #[inline]
    pub fn move_active_cursor(&mut self, direction: Direction, amt: u32) {
        self.move_cursor(self.tree.active(), direction, amt);
    }

    #[inline]
    pub fn active_cursor(&self) -> Position {
        self.active_view().cursor()
    }

    #[inline]
    pub fn set_active_cursor(&mut self, cursor: impl Into<Position>) {
        self.set_cursor(self.tree.active(), cursor);
    }
}
