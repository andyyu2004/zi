use tui::Rect;

use crate::view::HasViewId;
use crate::{Editor, Size, ViewId};

pub struct ViewTree {
    size: Size,
    layers: Vec<Layer>,
}

impl ViewTree {
    pub fn new(size: Size, view: ViewId) -> Self {
        ViewTree { size, layers: vec![Layer::new(size, view)] }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn area(&self, view: impl HasViewId) -> Rect {
        self.layers.last().expect("layers was empty").area(view)
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub fn push(&mut self, layer: Layer) {
        self.layers.push(layer);
    }

    pub fn pop(&mut self) -> Layer {
        self.layers.pop().expect("no layers to pop")
    }

    pub fn active(&self) -> ViewId {
        self.layers.last().expect("layers was empty").active_view()
    }

    pub fn close_active(&mut self) -> ViewId {
        let layer = self.layers.last_mut().expect("layers was empty");
        // FIXME this implementation is assuming each layer is one view
        let active = layer.active_view();
        self.pop();
        active
    }

    pub fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        for layer in &self.layers {
            layer.render(editor, area, surface);
        }
    }
}

pub struct Layer {
    view: ViewId,
    size: Size,
}

impl Layer {
    pub fn new(size: Size, view: ViewId) -> Self {
        Layer { size, view }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn area(&self, view: impl HasViewId) -> Rect {
        todo!()
    }

    pub fn active_view(&self) -> ViewId {
        self.view
    }

    fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        editor.view(self.view).render(editor, area, surface);
    }
}
