use zi_textobject::{motion, Motion};

use super::{get, mode, Selector};
use crate::view::SetCursorFlags;
use crate::{Direction, Editor, Point, ViewId};

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
        view.set_cursor_linewise(mode!(self), area, buf, pos.into(), SetCursorFlags::empty());
    }

    #[inline]
    pub fn move_cursor(
        &mut self,
        selector: impl Selector<ViewId>,
        direction: Direction,
        amt: u32,
    ) -> Point {
        let motion = match direction {
            Direction::Left => &motion::PrevChar as &dyn Motion,
            Direction::Right => &motion::NextChar,
            Direction::Up => &motion::PrevLine,
            Direction::Down => &motion::NextLine,
        }
        .repeat(amt as usize);
        self.motion(selector, motion)
    }

    #[inline]
    pub fn cursor(&self, selector: impl Selector<ViewId>) -> Point {
        self.view(selector).cursor()
    }
}
