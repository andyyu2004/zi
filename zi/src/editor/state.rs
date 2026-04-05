use crate::completion::Completion;
use crate::{Mode, Operator, Point};

/// Per mode state
#[derive(Debug)]
pub(super) enum State {
    Normal(NormalState),
    Insert(InsertState),
    Command(CommandState),
    Visual(VisualState),
    VisualLine(VisualLineState),
    VisualBlock(VisualBlockState),
    OperatorPending(OperatorPendingState),
    ReplacePending,
}

impl Default for State {
    fn default() -> Self {
        State::Normal(Default::default())
    }
}

impl State {
    pub(super) fn new(mode: Mode) -> Self {
        match mode {
            Mode::Normal => State::Normal(Default::default()),
            Mode::Insert => State::Insert(Default::default()),
            Mode::Command => State::Command(Default::default()),
            Mode::Visual => State::Visual(Default::default()),
            Mode::VisualLine => State::VisualLine(Default::default()),
            Mode::VisualBlock => State::VisualBlock(Default::default()),
            Mode::OperatorPending(op) => State::OperatorPending(OperatorPendingState::new(op)),
            Mode::ReplacePending => State::ReplacePending,
        }
    }

    pub(super) fn mode(&self) -> Mode {
        match self {
            State::Normal(..) => Mode::Normal,
            State::Insert(..) => Mode::Insert,
            State::Command(..) => Mode::Command,
            State::Visual(..) => Mode::Visual,
            State::VisualLine(..) => Mode::VisualLine,
            State::VisualBlock(..) => Mode::VisualBlock,
            State::OperatorPending(state) => Mode::OperatorPending(state.operator),
            State::ReplacePending => Mode::ReplacePending,
        }
    }

    pub(super) fn visual_anchor(&self) -> Option<Point> {
        match self {
            State::Visual(s) => Some(s.anchor),
            State::VisualLine(s) => Some(s.anchor),
            State::VisualBlock(s) => Some(s.anchor),
            _ => None,
        }
    }

    pub(super) fn set_visual_anchor(&mut self, anchor: Point) {
        match self {
            State::Visual(s) => s.anchor = anchor,
            State::VisualLine(s) => s.anchor = anchor,
            State::VisualBlock(s) => s.anchor = anchor,
            _ => {}
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct NormalState {}

#[derive(Debug, Default)]
pub(super) struct InsertState {
    pub(super) completion: Completion,
}

#[derive(Debug)]
pub(super) struct CommandState {
    /// Stores the command currently in the command line
    pub(super) buffer: String,
}

impl CommandState {
    pub(super) fn buffer(&self) -> &str {
        &self.buffer
    }
}

impl Default for CommandState {
    fn default() -> Self {
        Self { buffer: String::from(":") }
    }
}

#[derive(Debug)]
pub(super) struct VisualState {
    pub(super) anchor: Point,
}

impl Default for VisualState {
    fn default() -> Self {
        Self { anchor: Point::default() }
    }
}

#[derive(Debug)]
pub(super) struct VisualLineState {
    pub(super) anchor: Point,
}

impl Default for VisualLineState {
    fn default() -> Self {
        Self { anchor: Point::default() }
    }
}

#[derive(Debug)]
pub(super) struct VisualBlockState {
    pub(super) anchor: Point,
}

impl Default for VisualBlockState {
    fn default() -> Self {
        Self { anchor: Point::default() }
    }
}

#[derive(Debug)]
pub(super) struct OperatorPendingState {
    pub(crate) operator: Operator,
}

impl OperatorPendingState {
    pub(super) fn new(operator: Operator) -> Self {
        Self { operator }
    }
}
