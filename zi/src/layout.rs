use tui::Rect;

use crate::view::HasViewId;
use crate::{Direction, Editor, Size, ViewId};

pub struct ViewTree {
    size: Size,
    layers: Vec<Layer>,
}

impl ViewTree {
    pub fn new(size: Size, view: ViewId) -> Self {
        ViewTree { size, layers: vec![Layer::new(size, view)] }
    }

    fn top(&self) -> &Layer {
        self.layers.last().expect("layers was empty")
    }

    fn top_mut(&mut self) -> &mut Layer {
        self.layers.last_mut().expect("layers was empty")
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn area(&self, view: impl HasViewId) -> Rect {
        self.top().area(view)
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
        self.top().active_view()
    }

    pub fn close_active(&mut self) -> ViewId {
        let layer = self.top_mut();
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

    pub fn split(&mut self, view: ViewId, new: ViewId, direction: Direction) {
        self.top_mut().split(view, new, direction)
    }
}

pub struct Layer {
    active: ViewId,
    size: Size,
    root: Node,
}

impl Layer {
    pub fn new(size: Size, active: ViewId) -> Self {
        Layer { size, active, root: Node::View(active) }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn area(&self, view: impl HasViewId) -> Rect {
        self.root.area(view.view_id(), self.size)
    }

    pub fn split(&mut self, view: ViewId, new: ViewId, direction: Direction) {
        todo!()
    }

    pub fn active_view(&self) -> ViewId {
        self.active
    }

    fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        self.root.render(editor, area, surface);
    }
}

enum Node {
    View(ViewId),
    Container(Container),
}

impl Node {
    fn area(&self, view: ViewId, size: Size) -> Rect {
        match self {
            Node::View(id) => {
                assert_eq!(*id, view, "true for now");
                Rect::new(0, 0, size.width, size.height)
            }
            Node::Container(_container) => todo!(),
        }
    }

    fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        match self {
            Node::View(id) => editor.view(*id).render(editor, area, surface),
            Node::Container(_container) => {
                todo!()
            }
        }
    }
}

struct Container {
    direction: Direction,
    children: Vec<Node>,
}
