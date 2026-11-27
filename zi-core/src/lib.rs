pub mod completion;
pub mod dirs;
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
    VisualLine,
    VisualBlock,
    OperatorPending(Operator),
    ReplacePending,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Mode::Normal | Mode::OperatorPending(_) | Mode::ReplacePending => "",
                Mode::Command => "COMMAND",
                Mode::Insert => "INSERT",
                Mode::Visual => "VISUAL",
                Mode::VisualLine => "VISUAL LINE",
                Mode::VisualBlock => "VISUAL BLOCK",
            }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operator {
    Delete,
    Change,
    Yank,
}

rel::key! {
    pub struct ViewId;
    pub struct ViewGroupId;
    pub struct BufferId;
    pub struct NamespaceId;
}
