use super::get;
use crate::view::HasViewId;
use crate::{Direction, Editor, Point};

bitflags::bitflags! {
    // A bunch of hacks, don't make this public
    pub(crate) struct SetCursorFlags: u8 {
        const NO_COLUMN_BOUNDS_CHECK = 1 << 0;
        const MOVE_TO_LAST_LINE_IF_OUT_OF_BOUNDS = 1 << 1;
    }
}

impl Editor {
    #[inline]
    pub fn set_cursor(&mut self, view: impl HasViewId, pos: impl Into<Point>) {
        let (view, buf) = get!(self: view);
        let area = self.tree.view_area(view.id());
        view.set_cursor(self.mode, area, buf, pos.into(), SetCursorFlags::empty());
    }

    #[inline]
    pub fn move_cursor(&mut self, view: impl HasViewId, direction: Direction, amt: u32) -> Point {
        let (view, buf) = get!(self: view);
        let area = self.tree.view_area(view.id());
        view.move_cursor(self.mode, area, buf, direction, amt)
    }

    #[inline]
    pub fn move_active_cursor(&mut self, direction: Direction, amt: u32) {
        self.move_cursor(self.tree.active(), direction, amt);
    }

    #[inline]
    pub fn active_cursor(&self) -> Point {
        self.active_view().cursor()
    }

    #[inline]
    pub fn set_active_cursor(&mut self, cursor: impl Into<Point>) {
        self.set_cursor(self.tree.active(), cursor);
    }
}
