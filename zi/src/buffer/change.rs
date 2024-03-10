use std::borrow::Cow;

use smallvec::SmallVec;

use crate::{LazyText, Point, Range};

pub struct Change<'a> {
    operations: SmallVec<Operation<'a>, 1>,
}

impl<'a> Change<'a> {
    pub(crate) fn new(operations: impl Into<SmallVec<Operation<'a>, 1>>) -> Self {
        // todo validations etc
        Self { operations: operations.into() }
    }

    pub fn insert(pos: impl Into<Position>, text: impl Into<Cow<'a, str>>) -> Self {
        Self::new([Operation::Insert(pos.into(), text.into())])
    }

    pub fn operations(&self) -> &[Operation<'a>] {
        &self.operations
    }
}

#[derive(Debug)]
pub enum Operation<'a> {
    Insert(Position, Cow<'a, str>),
    Append(Cow<'a, str>),
    Delete(Range),
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Position {
    Char(usize),
    Point(Point),
}

impl Position {
    pub fn char_idx(self, text: &dyn LazyText) -> usize {
        match self {
            Self::Char(idx) => idx,
            Self::Point(point) => text.point_to_char(point),
        }
    }
}

impl From<Point> for Position {
    fn from(v: Point) -> Self {
        Self::Point(v)
    }
}

impl From<usize> for Position {
    fn from(v: usize) -> Self {
        Self::Char(v)
    }
}
