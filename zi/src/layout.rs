// use ratatui::layout::Layout;

use tui::Rect;

use crate::component::{Component, Surface};
use crate::view::HasViewId;
use crate::{Editor, Size, ViewId};

pub struct Tree {
    layers: Vec<Layer>,
    size: Size,
}

impl Tree {
    pub fn new(size: Size, root: impl Into<Node>) -> Self {
        Self { layers: vec![Layer::new(root)], size }
    }

    pub fn size(&self, _view: impl HasViewId) -> Size {
        self.size
    }

    pub fn push_layer(&mut self, layer: impl Into<Layer>) {
        self.layers.push(layer.into());
    }
}

impl Component for Tree {
    fn render(&self, editor: &Editor, area: Rect, surface: &mut Surface) {
        for layer in &self.layers {
            layer.render(editor, area, surface);
        }
    }
}

pub struct Layer {
    root: Node,
}

impl Layer {
    pub fn new(root: impl Into<Node>) -> Self {
        Self { root: root.into() }
    }
}

impl Component for Layer {
    fn render(&self, editor: &Editor, area: Rect, surface: &mut Surface) {
        match &self.root {
            Node::View(v) => editor.view(*v).render(editor, area, surface),
            Node::Component(c) => c.render(editor, area, surface),
        }
    }
}

pub enum Node {
    View(ViewId),
    Component(Box<dyn Component + Send>),
    // Container(Container),
}

impl<C> From<C> for Node
where
    C: Component + Send + 'static,
{
    fn from(value: C) -> Self {
        Self::Component(Box::new(value))
    }
}

impl From<ViewId> for Node {
    fn from(v: ViewId) -> Self {
        Self::View(v)
    }
}

// pub struct Container {
//     layout: Layout,
//     children: Vec<Node>,
// }
