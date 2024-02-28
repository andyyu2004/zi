pub(crate) use ratatui::buffer::Buffer as Surface;
use tui::Rect;

use crate::Editor;

pub trait Component {
    fn render(&self, _editor: &Editor, area: Rect, surface: &mut Surface);
}

pub(crate) struct Picker {}

impl Picker {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Component for Picker {
    fn render(&self, _editor: &Editor, _area: Rect, _surface: &mut Surface) {}
}
