use crate::{Mode, Operator};

#[derive(Debug)]
pub(super) enum State {
    Normal(NormalState),
    Insert(InsertState),
    Command(CommandState),
    Visual(VisualState),
    OperatorPending(OperatorPendingState),
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
            Mode::OperatorPending(op) => State::OperatorPending(OperatorPendingState::new(op)),
        }
    }

    pub(super) fn mode(&self) -> Mode {
        match self {
            State::Normal(..) => Mode::Normal,
            State::Insert(..) => Mode::Insert,
            State::Command(..) => Mode::Command,
            State::Visual(..) => Mode::Visual,
            State::OperatorPending(state) => Mode::OperatorPending(state.operator),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct NormalState {}

#[derive(Debug, Default)]
pub(super) struct InsertState {}

#[derive(Debug)]
pub(super) struct CommandState {
    /// Stores the command currently in the command line
    pub buffer: String,
}

impl Default for CommandState {
    fn default() -> Self {
        Self { buffer: String::from(":") }
    }
}

#[derive(Debug, Default)]
pub(super) struct VisualState {}

#[derive(Debug)]
pub(super) struct OperatorPendingState {
    operator: Operator,
}

impl OperatorPendingState {
    pub(super) fn new(operator: Operator) -> Self {
        Self { operator }
    }
}
