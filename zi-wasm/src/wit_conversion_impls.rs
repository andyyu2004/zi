use zi::Point;

use crate::wit::exports::zi::api::command;
use crate::wit::zi::api;

impl From<zi::Mode> for api::editor::Mode {
    fn from(mode: zi::Mode) -> Self {
        match mode {
            zi::Mode::Normal => api::editor::Mode::Normal,
            zi::Mode::Insert => api::editor::Mode::Insert,
            zi::Mode::Command => api::editor::Mode::Command,
            zi::Mode::Visual => api::editor::Mode::Visual,
            zi::Mode::OperatorPending(op) => api::editor::Mode::OperatorPending(op.into()),
            zi::Mode::ReplacePending => api::editor::Mode::ReplacePending,
        }
    }
}

impl From<api::editor::Mode> for zi::Mode {
    fn from(mode: api::editor::Mode) -> Self {
        match mode {
            api::editor::Mode::Normal => zi::Mode::Normal,
            api::editor::Mode::Insert => zi::Mode::Insert,
            api::editor::Mode::Command => zi::Mode::Command,
            api::editor::Mode::Visual => zi::Mode::Visual,
            api::editor::Mode::OperatorPending(op) => zi::Mode::OperatorPending(op.into()),
            api::editor::Mode::ReplacePending => zi::Mode::ReplacePending,
        }
    }
}

impl From<zi::Operator> for api::editor::Operator {
    fn from(op: zi::Operator) -> Self {
        match op {
            zi::Operator::Change => api::editor::Operator::Change,
            zi::Operator::Delete => api::editor::Operator::Delete,
            zi::Operator::Yank => api::editor::Operator::Yank,
        }
    }
}

impl From<api::editor::Operator> for zi::Operator {
    fn from(op: api::editor::Operator) -> Self {
        match op {
            api::editor::Operator::Change => zi::Operator::Change,
            api::editor::Operator::Delete => zi::Operator::Delete,
            api::editor::Operator::Yank => zi::Operator::Yank,
        }
    }
}

impl From<zi::EditError> for api::editor::EditError {
    fn from(err: zi::EditError) -> Self {
        match err {
            zi::EditError::Readonly => api::editor::EditError::Readonly,
        }
    }
}

impl From<Point> for api::editor::Point {
    fn from(value: Point) -> Self {
        Self { line: value.line() as u32, col: value.col() as u32 }
    }
}

impl From<api::editor::Point> for Point {
    fn from(value: api::editor::Point) -> Self {
        Self::from((value.line as usize, value.col as usize))
    }
}

impl From<api::editor::Direction> for zi::Direction {
    fn from(dir: api::editor::Direction) -> Self {
        match dir {
            api::editor::Direction::Left => zi::Direction::Left,
            api::editor::Direction::Right => zi::Direction::Right,
            api::editor::Direction::Up => zi::Direction::Up,
            api::editor::Direction::Down => zi::Direction::Down,
        }
    }
}

impl From<api::editor::VerticalAlignment> for zi::VerticalAlignment {
    fn from(align: api::editor::VerticalAlignment) -> Self {
        match align {
            api::editor::VerticalAlignment::Top => zi::VerticalAlignment::Top,
            api::editor::VerticalAlignment::Center => zi::VerticalAlignment::Center,
            api::editor::VerticalAlignment::Bottom => zi::VerticalAlignment::Bottom,
        }
    }
}

impl From<zi::BufferFlags> for api::editor::BufferFlags {
    fn from(flags: zi::BufferFlags) -> Self {
        let mut out = api::editor::BufferFlags::empty();
        if flags.contains(zi::BufferFlags::READONLY) {
            out |= api::editor::BufferFlags::READONLY;
        }
        if flags.contains(zi::BufferFlags::DIRTY) {
            out |= api::editor::BufferFlags::DIRTY;
        }
        out
    }
}

impl From<command::Arity> for zi::command::Arity {
    fn from(arity: command::Arity) -> Self {
        zi::command::Arity { min: arity.min, max: arity.max }
    }
}

impl From<command::CommandFlags> for zi::command::CommandFlags {
    fn from(flags: command::CommandFlags) -> Self {
        // Ideally don't need transmute, but don't see a better non-manual way to do this conversion currently.
        zi::command::CommandFlags::from_bits(unsafe {
            std::mem::transmute::<crate::wit::exports::zi::api::command::CommandFlags, u8>(flags)
        })
        .expect("invalid command flags")
    }
}
