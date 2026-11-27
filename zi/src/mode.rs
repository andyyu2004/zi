use std::fmt;

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
