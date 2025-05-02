pub mod completion;
mod position;
pub mod style;

use std::fmt;

pub use self::completion::CompletionItem;
pub use self::position::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Visual,
    OperatorPending(Operator),
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", match self {
            Mode::Normal | Mode::OperatorPending(_) => "",
            Mode::Command => "COMMAND",
            Mode::Insert => "INSERT",
            Mode::Visual => "VISUAL",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operator {
    Delete,
    Change,
    Yank,
}

slotmap::new_key_type! {
    pub struct ViewId;
    pub struct ViewGroupId;
    pub struct BufferId;
    pub struct NamespaceId;
}
