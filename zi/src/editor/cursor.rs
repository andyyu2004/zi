use zi_core::PointOrByte;
use zi_textobject::{motion, Motion};

use super::{get, get_ref, mode, Selector};
use crate::view::SetCursorFlags;
use crate::{Direction, Editor, Mode, Point, ViewId};

impl Editor {
    #[inline]
    pub fn cursor(&self, selector: impl Selector<ViewId>) -> Point {
        self.view(selector).cursor()
    }

    pub fn cursor_byte(&self, selector: impl Selector<ViewId>) -> usize {
        let view = selector.select(self);
        let (view, buf) = get_ref!(self: view);
        let point = view.cursor();
        buf.text().point_to_byte(point)
    }

    #[inline]
    pub fn set_cursor(&mut self, selector: impl Selector<ViewId>, pos: impl Into<PointOrByte>) {
        match pos.into() {
            PointOrByte::Point(pos) => {
                self.set_cursor_flags(selector, pos, SetCursorFlags::empty())
            }
            PointOrByte::Byte(byte) => self.set_cursor_bytewise(selector, byte),
        }
    }

    #[inline]
    pub(crate) fn set_cursor_flags(
        &mut self,
        selector: impl Selector<ViewId>,
        pos: impl Into<Point>,
        flags: SetCursorFlags,
    ) {
        let view_id = selector.select(self);
        let (view, buf) = get!(self: view_id);
        let area = self.tree.view_area(view.id());
        view.set_cursor_linewise(mode!(self), area, buf, pos.into(), flags);
    }

    #[inline]
    pub fn move_cursor(
        &mut self,
        selector: impl Selector<ViewId>,
        direction: Direction,
        amt: usize,
    ) -> Point {
        let motion = match direction {
            Direction::Left => &motion::PrevChar as &dyn Motion,
            Direction::Right => &motion::NextChar,
            Direction::Up => &motion::PrevLine,
            Direction::Down => &motion::NextLine,
        }
        .repeat(amt);

        if let Mode::OperatorPending(_) = self.mode() {
            self.set_mode(Mode::Normal);
        }

        self.motion(selector, motion).expect("this only returns errors in operator-pending mode")
    }

    #[inline]
    pub fn set_cursor_bytewise(&mut self, selector: impl Selector<ViewId>, byte: usize) {
        let view_id = selector.select(self);
        let (view, buf) = get!(self: view_id);
        let area = self.tree.view_area(view.id());
        view.set_cursor_bytewise(mode!(self), area, buf, byte, SetCursorFlags::empty());
    }
}
