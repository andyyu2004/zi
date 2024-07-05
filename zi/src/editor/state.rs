use std::cell::RefCell;

use nucleo::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo::Utf32Str;
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
pub(super) enum CompletionState {
    Active(ActiveCompletionState),
    #[default]
    Inactive,
}

#[derive(Debug, Default)]
pub(super) struct ActiveCompletionState {
    pub widget_state: RefCell<tui::ListState>,
    options: Vec<CompletionItem>,
    matches: Vec<nucleo::Match>,
    matcher: nucleo::Matcher,
    query: String,
}

impl CompletionState {
    pub(super) fn deactivate(&mut self) {
        *self = CompletionState::Inactive;
    }

    pub(super) fn activate(&mut self) {
        if let CompletionState::Inactive = self {
            *self = CompletionState::Active(Default::default());
        }
    }

    pub(super) fn matches(&self) -> impl ExactSizeIterator<Item = &CompletionItem> {
        match self {
            CompletionState::Inactive => {
                Box::new([].iter()) as Box<dyn ExactSizeIterator<Item = &CompletionItem>>
            }
            CompletionState::Active(state) => Box::new(state.matches()),
        }
    }
}

impl ActiveCompletionState {
    pub(super) fn set_items(&mut self, options: Vec<CompletionItem>) {
        self.options = options;
        self.compute_matches();
    }

    pub(super) fn update_query(&mut self, c: Option<char>) {
        match c {
            Some(c) => self.query.push(c),
            None => {
                self.query.pop();
            }
        }

        self.compute_matches();
    }

    pub fn matches(&self) -> impl ExactSizeIterator<Item = &CompletionItem> {
        if self.matches.is_empty() {
            Box::new(self.options.iter()) as Box<dyn ExactSizeIterator<Item = &CompletionItem>>
        } else {
            Box::new(self.matches.iter().map(|m| &self.options[m.idx as usize]))
        }
    }

    fn compute_matches(&mut self) {
        let pattern = Atom::new(
            &self.query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
            false,
        );

        let mut buf = vec![];
        self.matches.clear();
        let matches = self.options.iter().enumerate().filter_map(|(idx, item)| {
            pattern
                .score(Utf32Str::new(&item.label, &mut buf), &mut self.matcher)
                .map(|score| nucleo::Match { idx: idx as u32, score: score as u32 })
        });
        self.matches.extend(matches);

        self.widget_state.borrow_mut().select(None);
    }
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
