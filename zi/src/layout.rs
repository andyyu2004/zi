// use ratatui::layout::Layout;

use crate::view::HasViewId;
use crate::{Size, ViewId};

pub struct Tree {
    size: Size,
    _root: Node,
}

impl Tree {
    pub fn new(size: Size, root: impl Into<Node>) -> Self {
        Self { size, _root: root.into() }
    }

    pub fn size(&self, _view: impl HasViewId) -> Size {
        self.size
    }
}

pub enum Node {
    View(ViewId),
    // Container(Container),
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
