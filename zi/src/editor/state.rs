use super::Match;
use crate::{Mode, Operator};

/// Shared state
#[derive(Default)]
pub(super) struct SharedState {
    matches: Vec<Match>,
    pub(super) show_search_hl: bool,
    match_idx: usize,
}

impl SharedState {
    pub(super) fn matches(&self) -> &[Match] {
        &self.matches
    }

    pub(super) fn set_matches(&mut self, matches: impl Into<Vec<Match>>) {
        self.matches = matches.into();
        self.match_idx = 0;
    }

    pub(super) fn current_match_idx(&self) -> usize {
        self.match_idx
    }

    pub(super) fn current_match(&self) -> Option<&Match> {
        self.matches.get(self.match_idx)
    }

    pub(super) fn next_match(&mut self) -> Option<&Match> {
        // TODO test for division by zero
        if self.matches.is_empty() {
            return None;
        }

        self.match_idx = (self.match_idx + 1) % self.matches.len();
        self.matches.get(self.match_idx)
    }

    pub(super) fn prev_match(&mut self) -> Option<&Match> {
        if self.match_idx == 0 {
            self.match_idx = self.matches.len() - 1;
        } else {
            self.match_idx -= 1;
        }

        self.matches.get(self.match_idx)
    }
}

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
pub(super) struct InsertState {}

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
    pub operator: Operator,
}

impl OperatorPendingState {
    pub(super) fn new(operator: Operator) -> Self {
        Self { operator }
    }
}
