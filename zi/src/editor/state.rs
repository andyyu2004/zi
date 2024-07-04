use std::cell::RefCell;

use zi_core::CompletionItem;

use crate::{Mode, Operator};

/// Per mode state
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

    pub(super) fn transition(self, to: Mode) -> State {
        // any state specific code can be done here
        State::new(to)
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
pub(super) struct InsertState {
    pub(super) completion: CompletionState,
}

#[derive(Debug, Default)]
pub(super) struct CompletionState {
    pub(super) show: bool,
    pub(super) items: Vec<CompletionItem>,
    pub(super) widget_state: RefCell<tui::ListState>,
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

#[derive(Debug, Default)]
pub(super) struct VisualState {}

#[derive(Debug)]
pub(super) struct OperatorPendingState {
    pub(crate) operator: Operator,
}

impl OperatorPendingState {
    pub(super) fn new(operator: Operator) -> Self {
        Self { operator }
    }
}
