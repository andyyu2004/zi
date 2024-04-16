use super::{get, Selector};
use crate::{Direction, Editor, Point, ViewId};

bitflags::bitflags! {
    // A bunch of hacks, don't make this public
    pub(crate) struct SetCursorFlags: u8 {
        const NO_COLUMN_BOUNDS_CHECK = 1 << 0;
    }
}

impl Editor {
    #[inline]
    pub fn get_cursor(&self, view: impl Selector<ViewId>) -> Point {
        self.view(view).cursor()
    }

    #[inline]
    pub fn set_cursor(&mut self, selector: impl Selector<ViewId>, pos: impl Into<Point>) {
        let view_id = selector.select(self);
        let (view, buf) = get!(self: view_id);
        let area = self.tree.view_area(view.id());
        view.set_cursor_linewise(self.mode, area, buf, pos.into(), SetCursorFlags::empty());
    }


    #[inline]
    pub fn move_cursor(
        &mut self,
        selector: impl Selector<ViewId>,
        direction: Direction,
        amt: u32,
    ) -> Point {
        let view_id = selector.select(self);
        let (view, buf) = get!(self: view_id);
        let area = self.tree.view_area(view.id());
        view.move_cursor(self.mode, area, buf, direction, amt)
    }

    #[inline]
    pub fn cursor(&self, selector: impl Selector<ViewId>) -> Point {
        self.view(selector).cursor()
    }
}
