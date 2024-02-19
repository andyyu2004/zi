use super::*;

impl Editor {
    #[inline]
    pub fn set_cursor(&mut self, view: impl HasViewId, cursor: impl Into<Position>) {
        let view = &mut self.views[view.view_id()];
        let buf = &self.buffers[view.buffer()];

        view.set_cursor(buf, cursor.into());
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
