use tui::{Constraint, Layout, Rect};

use crate::view::HasViewId;
use crate::{Direction, Editor, Size, ViewId};

pub struct ViewTree {
    size: Size,
    layers: Vec<Layer>,
}

impl ViewTree {
    pub fn new(size: Size, view: ViewId) -> Self {
        ViewTree { size, layers: vec![Layer::new(Rect::new(0, 0, size.width, size.height), view)] }
    }

    fn top(&self) -> &Layer {
        self.layers.last().expect("layers was empty")
    }

    fn top_mut(&mut self) -> &mut Layer {
        self.layers.last_mut().expect("layers was empty")
    }

    pub fn area(&self) -> Rect {
        Rect::new(0, 0, self.size.width, self.size.height)
    }

    pub fn view_area(&self, view: impl HasViewId) -> Rect {
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

#[derive(Debug)]
pub struct Layer {
    active: ViewId,
    area: Rect,
    root: Node,
}

impl Layer {
    pub fn new(area: Rect, active: ViewId) -> Self {
        Layer { area, active, root: Node::View(active) }
    }

    pub fn area(&self, view: impl HasViewId) -> Rect {
        self.root.area(view.view_id(), self.area).expect("view not found in layer")
    }

    pub fn split(&mut self, view: ViewId, new: ViewId, direction: Direction) {
        self.root.split(view, new, direction);
        self.active = new;
    }

    pub fn active_view(&self) -> ViewId {
        self.active
    }

    fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        self.root.render(editor, area, surface);
    }
}

#[derive(Debug)]
enum Node {
    View(ViewId),
    Container(Container),
}

impl Node {
    fn area(&self, view: ViewId, area: Rect) -> Option<Rect> {
        match self {
            Node::View(id) => {
                if *id == view {
                    return Some(area);
                }
            }
            Node::Container(container) => return Some(container.area(view, area)),
        }

        None
    }

    fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        match self {
            Node::View(id) => editor.view(*id).render(editor, area, surface),
            Node::Container(container) => container.render(editor, area, surface),
        }
    }

    fn split(&mut self, view: ViewId, new: ViewId, direction: Direction) {
        match self {
            Node::View(v) => {
                assert_eq!(*v, view, "true for now");
                let children = match direction {
                    Direction::Left | Direction::Up => vec![Node::View(new), Node::View(view)],
                    Direction::Right | Direction::Down => vec![Node::View(view), Node::View(new)],
                };

                *self = Node::Container(Container {
                    layout: Layout::new(
                        direction.into(),
                        [Constraint::Fill(1), Constraint::Fill(1)],
                    ),
                    children,
                })
            }
            Node::Container(_) => todo!(),
        }
    }
}

#[derive(Debug)]
struct Container {
    layout: Layout,
    children: Vec<Node>,
}

impl Container {
    pub fn new(
        direction: impl Into<tui::Direction>,
        children: impl IntoIterator<Item = (Constraint, Node)>,
    ) -> Self {
        let (constraints, children): (Vec<_>, Vec<_>) = children.into_iter().unzip();
        let layout = Layout::new(direction.into(), constraints);
        Container { layout, children }
    }

    pub fn area(&self, view: ViewId, area: Rect) -> Rect {
        let areas = self.layout.split(area);
        assert_eq!(areas.len(), self.children.len());
        for (&area, child) in areas.iter().zip(&self.children) {
            if let Some(area) = child.area(view, area) {
                return area;
            }
        }

        panic!("view not found")
    }

    pub fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        let areas = self.layout.split(area);
        assert_eq!(areas.len(), self.children.len());
        for (&area, child) in areas.into_iter().zip(&self.children) {
            child.render(editor, area, surface);
        }
    }
}

#[cfg(test)]
mod tests;
