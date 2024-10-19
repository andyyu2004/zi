use crate::wit::zi::api;

impl From<zi::Mode> for api::editor::Mode {
    fn from(mode: zi::Mode) -> Self {
        match mode {
            zi::Mode::Normal => api::editor::Mode::Normal,
            zi::Mode::Insert => api::editor::Mode::Insert,
            zi::Mode::Command => api::editor::Mode::Command,
            zi::Mode::Visual => api::editor::Mode::Visual,
            zi::Mode::OperatorPending(op) => api::editor::Mode::OperatorPending(op.into()),
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
