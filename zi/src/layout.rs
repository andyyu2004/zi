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
        self.top().view_area(view)
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    pub fn push(&mut self, layer: Layer) {
        self.layers.push(layer);
    }

    pub fn pop(&mut self) {
        self.layers.pop().expect("no layers to pop");
    }

    pub fn active(&self) -> ViewId {
        self.top().active_view()
    }

    pub fn close_active(&mut self) -> ViewId {
        let layer = self.top_mut();
        let view = layer.active_view();
        match layer.close_view(view) {
            CloseResult::Continue => unreachable!("close_active_view should always remove a view"),
            CloseResult::Done(..) => (),
            // pop the entire layer as it's empty
            CloseResult::RemoveFromParent => self.pop(),
        };
        view
    }

    pub(crate) fn views(&self) -> impl Iterator<Item = ViewId> + '_ {
        self.layers.iter().flat_map(|layer| layer.views())
    }

    pub fn render(&self, editor: &Editor, surface: &mut tui::Buffer) {
        for layer in &self.layers {
            layer.render(editor, surface);
        }
    }

    pub fn split(&mut self, view: ViewId, new: ViewId, direction: Direction) {
        assert_ne!(view, new, "cannot split a view into itself");
        assert!(self.views().all(|v| v != new), "cannot split into an existing view");
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

    pub fn view_area(&self, view: impl HasViewId) -> Rect {
        self.root.view_area(view.view_id(), self.area).expect("view not found in layer")
    }

    pub fn split(&mut self, view: ViewId, new: ViewId, direction: Direction) {
        self.root.split(view, new, direction);
        self.active = new;
    }

    pub fn active_view(&self) -> ViewId {
        self.active
    }

    fn views(&self) -> impl Iterator<Item = ViewId> + '_ {
        self.root.views()
    }

    fn render(&self, editor: &Editor, surface: &mut tui::Buffer) {
        self.root.render(editor, self.area, surface);
    }

    fn close_view(&mut self, view: ViewId) -> CloseResult {
        let res = self.root.close_view(view);
        if let CloseResult::Done(next_active) = res {
            self.active = next_active;
        }
        res
    }
}

#[derive(Debug)]
enum Node {
    View(ViewId),
    Container(Container),
}

impl Node {
    fn view_area(&self, view: ViewId, area: Rect) -> Option<Rect> {
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
                assert_eq!(*v, view);
                let children = match direction {
                    Direction::Left | Direction::Up => vec![Node::View(new), Node::View(view)],
                    Direction::Right | Direction::Down => {
                        vec![Node::View(view), Node::View(new)]
                    }
                };

                *self = Node::Container(Container::new(
                    direction,
                    children.into_iter().map(|n| (Constraint::Fill(1), n)),
                ));
            }
            Node::Container(container) => container.split(view, new, direction),
        }
    }

    fn views(&self) -> impl Iterator<Item = ViewId> + '_ {
        match self {
            Node::View(id) => Box::new(std::iter::once(*id)) as Box<dyn Iterator<Item = ViewId>>,
            Node::Container(container) => Box::new(container.views()),
        }
    }

    fn close_view(&mut self, view: ViewId) -> CloseResult {
        match self {
            Node::View(v) if *v == view => CloseResult::RemoveFromParent,
            Node::Container(c) => c.close_view(view),
            _ => CloseResult::Continue,
        }
    }

    fn first_view(&self) -> ViewId {
        match self {
            Node::View(id) => *id,
            Node::Container(c) => c.first_view(),
        }
    }
}

#[must_use]
enum CloseResult {
    Continue,
    RemoveFromParent,
    /// The view was removed, and the next active view is the given one
    Done(ViewId),
}

#[derive(Debug)]
struct Container {
    // can't store tui::Layout directly because there's no way to access it's fields
    constraints: Vec<Constraint>,
    direction: tui::Direction,
    children: Vec<Node>,
}

impl Container {
    fn new(
        direction: impl Into<tui::Direction>,
        children: impl IntoIterator<Item = (Constraint, Node)>,
    ) -> Self {
        let (constraints, children): (Vec<_>, Vec<_>) = children.into_iter().unzip();
        Container { direction: direction.into(), constraints, children }
    }

    fn area(&self, view: ViewId, area: Rect) -> Rect {
        let areas = self.layout().split(area);
        assert_eq!(areas.len(), self.children.len());
        for (&area, child) in areas.iter().zip(&self.children) {
            if let Some(area) = child.view_area(view, area) {
                return area;
            }
        }

        panic!("view not found")
    }

    fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        let areas = self.layout().split(area);
        assert_eq!(areas.len(), self.children.len());
        for (&area, child) in areas.iter().zip(&self.children) {
            child.render(editor, area, surface);
        }
    }

    fn layout(&self) -> Layout {
        Layout::new(self.direction, self.constraints.clone())
    }

    fn insert(&mut self, idx: usize, new: ViewId, direction: Direction) {
        assert_eq!(
            self.direction,
            direction.into(),
            "cannot insert into container with a different direction"
        );

        let node = Node::View(new);
        let constraint = Constraint::Fill(1);
        match direction {
            Direction::Left | Direction::Up => {
                self.children.insert(idx, node);
                self.constraints.insert(idx, constraint);
            }
            Direction::Right | Direction::Down => {
                if idx + 1 < self.children.len() {
                    self.children.insert(idx + 1, node);
                    self.constraints.insert(idx + 1, constraint);
                } else {
                    self.children.push(node);
                    self.constraints.push(constraint);
                }
            }
        }

        assert_eq!(self.children.len(), self.constraints.len());
    }

    fn split(&mut self, view: ViewId, new: ViewId, direction: Direction) {
        // need manual loop to avoid borrowing issues
        for i in 0..self.children.len() {
            let child = &mut self.children[i];
            match child {
                Node::View(v) if *v == view => {
                    if self.direction == direction.into() {
                        // if the direction is the same, we can extend the container
                        self.insert(i, new, direction);
                    } else {
                        // otherwise, we create a new container
                        child.split(view, new, direction)
                    }
                }
                Node::Container(c) => c.split(view, new, direction),
                _ => continue,
            }
        }
    }

    fn views(&self) -> impl Iterator<Item = ViewId> + '_ {
        self.children.iter().flat_map(|child| child.views())
    }

    fn close_view(&mut self, view: ViewId) -> CloseResult {
        for i in 0..self.children.len() {
            let child = &mut self.children[i];
            match child.close_view(view) {
                CloseResult::Continue => continue,
                CloseResult::Done(next) => return CloseResult::Done(next),
                CloseResult::RemoveFromParent => {
                    if self.children.len() == 1 {
                        // The container is now empty, so we should remove it
                        return CloseResult::RemoveFromParent;
                    }

                    self.children.remove(i);
                    self.constraints.remove(i);

                    // First idea that came to mind, get the first child of the child before the removed one
                    let next_active = self.children[i.saturating_sub(1)].first_view();
                    return CloseResult::Done(next_active);
                }
            }
        }

        CloseResult::Continue
    }

    fn first_view(&self) -> ViewId {
        self.children.first().expect("container was empty").first_view()
    }
}
