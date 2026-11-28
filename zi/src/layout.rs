use std::cell::RefCell;
use std::collections::HashMap;

use tui::{Constraint, Layout, Rect, Widget as _};

use crate::{Direction, Editor, Size, ViewId};

pub(crate) struct ViewTree {
    size: Size,
    layers: Vec<Layer>,
    last_known_area: RefCell<HashMap<ViewId, Rect>>,
}

impl ViewTree {
    pub fn new(size: Size, view: ViewId) -> Self {
        ViewTree { size, layers: vec![Layer::new(view)], last_known_area: Default::default() }
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn area(&self) -> Rect {
        Rect::new(0, 0, self.size.width, self.size.height)
    }

    /// Get the area of a view in the tree, returns the last known area if the view is no longer in the tree
    pub fn view_area(&self, id: ViewId) -> Rect {
        for layer in self.layers.iter().rev() {
            if let Some(area) = layer.view_area(self.area(), id) {
                self.last_known_area.borrow_mut().insert(id, area);
                return area;
            }
        }

        self.last_known_area
            .borrow()
            .get(&id)
            .copied()
            .expect("view has never been in the view tree")
    }

    pub fn view_size(&self, id: ViewId) -> Size {
        let area = self.view_area(id);
        Size::new(area.width, area.height)
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

    pub fn close_view(&mut self, view: ViewId) -> Result<(), ()> {
        if self.is_empty() {
            return Err(());
        }

        let layer = self.top_mut();
        match layer.close_view(view) {
            TraverseResult::Continue => Err(()),
            TraverseResult::Done(..) => Ok(()),
            // pop the entire layer as it's empty
            TraverseResult::Propagate => {
                self.pop();
                Ok(())
            }
        }
    }

    pub fn render(&self, editor: &Editor, surface: &mut tui::Buffer) {
        for layer in &self.layers {
            layer.render(editor, self.area(), surface);
        }
    }

    pub fn split(
        &mut self,
        view: ViewId,
        new: ViewId,
        direction: Direction,
        constraint: Constraint,
    ) {
        assert_ne!(view, new, "cannot split a view into itself");
        assert!(self.views().all(|v| v != new), "cannot split into an existing view");
        self.top_mut().split(view, new, direction, constraint);
    }

    pub fn focus_direction(&mut self, direction: Direction) -> ViewId {
        self.top_mut().focus_direction(direction)
    }

    pub fn focus(&mut self, view: ViewId) {
        self.top_mut().focus(view);
    }

    pub fn views(&self) -> impl Iterator<Item = ViewId> + '_ {
        self.layers.iter().flat_map(Layer::views)
    }

    pub fn top(&self) -> &Layer {
        self.layers.last().expect("layers empty")
    }

    fn top_mut(&mut self) -> &mut Layer {
        self.layers.last_mut().expect("layers empty")
    }

    pub(crate) fn resize(&mut self, size: Size) {
        self.size = size;
    }

    pub(crate) fn view_only(&mut self, view: ViewId) {
        self.layers = vec![Layer::new(view)];
    }
}

pub struct Layer {
    active: ViewId,
    root: Node,
    compute_area: Box<dyn Fn(Rect) -> Rect + Send>,
}

impl Layer {
    pub fn new(active: ViewId) -> Self {
        Self::new_with_area(active, |area| area)
    }

    /// Create a new layer with a custom area function
    pub fn new_with_area(
        active: ViewId,
        compute_area: impl Fn(Rect) -> Rect + Send + 'static,
    ) -> Self {
        Layer { active, root: Node::View(active), compute_area: Box::new(compute_area) }
    }

    pub fn view_area(&self, area: Rect, view: ViewId) -> Option<Rect> {
        self.root.view_area((self.compute_area)(area), view)
    }

    pub fn split(
        &mut self,
        view: ViewId,
        new: ViewId,
        direction: Direction,
        constraint: Constraint,
    ) {
        let found = self.root.split(view, new, direction, constraint);
        assert!(found, "cannot split a view that doesn't exist in layer");
        self.active = new;
    }

    pub fn active_view(&self) -> ViewId {
        self.active
    }

    pub(crate) fn views(&self) -> impl Iterator<Item = ViewId> + '_ {
        self.root.views()
    }

    fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        let area = (self.compute_area)(area);
        tui::Clear.render(area, surface);
        self.root.render(editor, area, surface);
    }

    fn close_view(&mut self, view: ViewId) -> TraverseResult<ViewId> {
        let res = self.root.close_view(view);
        if let TraverseResult::Done(next_active) = res {
            self.active = next_active;
        }
        res
    }

    fn focus_direction(&mut self, direction: Direction) -> ViewId {
        self.active = match self.root.next_view(self.active, direction) {
            TraverseResult::Continue => panic!("active_view not found"),
            // This case only occurs when the root is a view, so the next view is always itself
            TraverseResult::Propagate => self.active,
            TraverseResult::Done(v) => v,
        };
        self.active
    }

    fn focus(&mut self, view: ViewId) {
        assert!(
            self.views().any(|v| v == view),
            "cannot focus on a view that doesn't exist in layer"
        );
        self.active = view;
    }
}

#[derive(Debug, Clone)]
enum Node {
    View(ViewId),
    Container(Container),
}

impl Node {
    fn view_area(&self, area: Rect, view: ViewId) -> Option<Rect> {
        match self {
            Node::View(id) if *id == view => Some(area),
            Node::Container(container) => container.area(area, view),
            Node::View(_) => None,
        }
    }

    fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        match self {
            Node::View(view) => editor.render_view(area, surface, *view),
            Node::Container(container) => container.render(editor, area, surface),
        }
    }

    fn split(
        &mut self,
        view: ViewId,
        new: ViewId,
        direction: Direction,
        constraint: Constraint,
    ) -> bool {
        match self {
            Node::View(v) if *v == view => {
                let children = match direction {
                    Direction::Left | Direction::Up => {
                        [(constraint, Node::View(new)), (Constraint::Fill(1), Node::View(view))]
                    }
                    Direction::Right | Direction::Down => {
                        [(Constraint::Fill(1), Node::View(view)), (constraint, Node::View(new))]
                    }
                };

                *self = Node::Container(Container::new(direction, children));
                true
            }
            Node::Container(container) => container.split(view, new, direction, constraint),
            Node::View(_) => false,
        }
    }

    fn views(&self) -> impl Iterator<Item = ViewId> + '_ {
        match self {
            Node::View(id) => Box::new(std::iter::once(*id)) as Box<dyn Iterator<Item = ViewId>>,
            Node::Container(container) => Box::new(container.views()),
        }
    }

    fn close_view(&mut self, view: ViewId) -> TraverseResult<ViewId> {
        match self {
            Node::View(v) if *v == view => TraverseResult::Propagate,
            Node::Container(c) => c.close_view(view),
            Node::View(_) => TraverseResult::Continue,
        }
    }

    fn first_view(&self) -> ViewId {
        match self {
            Node::View(id) => *id,
            Node::Container(c) => c.first_view(),
        }
    }

    fn next_view(&self, view: ViewId, direction: Direction) -> TraverseResult<ViewId> {
        match self {
            Node::View(v) if *v == view => TraverseResult::Propagate,
            Node::Container(c) => c.next_view(view, direction),
            Node::View(_) => TraverseResult::Continue,
        }
    }
}

#[must_use]
enum TraverseResult<T> {
    Continue,
    Propagate,
    Done(T),
}

#[derive(Debug, Clone)]
struct Container {
    // can't store tui::Layout directly because there's no way to access it's fields
    constraints: Vec<Constraint>,
    direction: tui::Direction,
    children: Vec<Node>,
}

impl Container {
    fn new(direction: Direction, children: impl IntoIterator<Item = (Constraint, Node)>) -> Self {
        let (constraints, children): (Vec<_>, Vec<_>) = children.into_iter().unzip();
        Container { direction: split_axis(direction), constraints, children }
    }

    fn area(&self, area: Rect, view: ViewId) -> Option<Rect> {
        let areas = self.layout().split(area);
        assert_eq!(areas.len(), self.children.len());
        for (&area, child) in areas.iter().zip(&self.children) {
            if let Some(area) = child.view_area(area, view) {
                return Some(area);
            }
        }

        None
    }

    fn render(&self, editor: &Editor, area: Rect, surface: &mut tui::Buffer) {
        let areas = self.layout().split(area);
        assert_eq!(areas.len(), self.children.len());
        for (&area, child) in areas.iter().zip(&self.children) {
            child.render(editor, area, surface);
        }
    }

    fn layout(&self) -> Layout {
        assert_eq!(self.constraints.len(), self.children.len());
        Layout::new(self.direction, self.constraints.clone())
    }

    fn insert(&mut self, idx: usize, new: ViewId, direction: Direction, constraint: Constraint) {
        assert_eq!(
            self.direction,
            split_axis(direction),
            "cannot insert into container with a different direction"
        );

        let node = Node::View(new);
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

    fn split(
        &mut self,
        view: ViewId,
        new: ViewId,
        direction: Direction,
        constraint: Constraint,
    ) -> bool {
        for i in 0..self.children.len() {
            let child = &mut self.children[i];
            match child {
                Node::View(v) if *v == view => {
                    if self.direction == split_axis(direction) {
                        self.insert(i, new, direction, constraint);
                        return true;
                    }
                    return child.split(view, new, direction, constraint);
                }
                Node::Container(c) => {
                    if c.split(view, new, direction, constraint) {
                        return true;
                    }
                }
                Node::View(_) => {}
            }
        }
        false
    }

    fn views(&self) -> impl Iterator<Item = ViewId> + '_ {
        self.children.iter().flat_map(Node::views)
    }

    fn close_view(&mut self, view: ViewId) -> TraverseResult<ViewId> {
        for i in 0..self.children.len() {
            let child = &mut self.children[i];
            match child.close_view(view) {
                TraverseResult::Continue => {}
                TraverseResult::Done(next) => return TraverseResult::Done(next),
                TraverseResult::Propagate => {
                    if self.children.len() == 1 {
                        // The container is now empty, so we should remove it
                        return TraverseResult::Propagate;
                    }

                    self.children.remove(i);
                    self.constraints.remove(i);

                    // First idea that came to mind, get the first child of the child before the removed one
                    let next_active = self.children[i.saturating_sub(1)].first_view();
                    return TraverseResult::Done(next_active);
                }
            }
        }

        TraverseResult::Continue
    }

    fn first_view(&self) -> ViewId {
        self.children.first().expect("container was empty").first_view()
    }

    fn next_view(&self, view: ViewId, direction: Direction) -> TraverseResult<ViewId> {
        for (i, child) in self.children.iter().enumerate() {
            match child.next_view(view, direction) {
                TraverseResult::Continue => {}
                TraverseResult::Done(next) => return TraverseResult::Done(next),
                TraverseResult::Propagate => {
                    if self.direction != split_axis(direction) {
                        // If the container direction is different, we try again in the parent container
                        return TraverseResult::Propagate;
                    }

                    // If the direction is the same, we can just move to the next/previous view
                    let next_idx = match direction {
                        Direction::Left | Direction::Up => {
                            if i == 0 {
                                return TraverseResult::Propagate;
                            }
                            i - 1
                        }
                        Direction::Right | Direction::Down => {
                            if i + 1 == self.children.len() {
                                return TraverseResult::Propagate;
                            }
                            i + 1
                        }
                    };

                    return TraverseResult::Done(self.children[next_idx].first_view());
                }
            }
        }

        TraverseResult::Continue
    }
}

fn split_axis(direction: Direction) -> tui::Direction {
    match direction {
        Direction::Left | Direction::Right => tui::Direction::Horizontal,
        Direction::Up | Direction::Down => tui::Direction::Vertical,
    }
}
