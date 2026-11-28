use super::{Active, Editor};
use crate::completion::Completion;
use crate::{BufferId, Mode, Operator, Point};

/// Per mode state
#[derive(Debug)]
pub(super) enum State {
    Normal(NormalState),
    Insert(InsertState),
    Command(CommandState),
    Visual(VisualState),
    VisualLine(VisualState),
    VisualBlock(VisualState),
    OperatorPending(OperatorPendingState),
    ReplacePending,
}

impl Default for State {
    fn default() -> Self {
        State::Normal(Default::default())
    }
}

impl State {
    pub(super) fn new(editor: &Editor, mode: Mode) -> Self {
        match mode {
            Mode::Normal => State::Normal(Default::default()),
            Mode::Insert => State::Insert(Default::default()),
            Mode::Command => State::Command(Default::default()),
            Mode::Visual => State::Visual(VisualState { anchor: editor.cursor(Active) }),
            Mode::VisualLine => State::VisualLine(VisualState { anchor: editor.cursor(Active) }),
            Mode::VisualBlock => State::VisualBlock(VisualState { anchor: editor.cursor(Active) }),
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
            State::Visual(s) | State::VisualLine(s) | State::VisualBlock(s) => Some(s.anchor),
            State::Normal(_)
            | State::Insert(_)
            | State::Command(_)
            | State::OperatorPending(_)
            | State::ReplacePending => None,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct NormalState {}

#[derive(Debug, Default)]
pub(super) struct InsertState {
    pub(super) completion: Completion,
    /// Buffer and end of the untouched indentation generated on the current insert.
    pub(super) autoindent: Option<(BufferId, Point)>,
    /// The count the insert started with. Leaving insert mode repeats `edits` to make it up.
    pub(super) count: Option<usize>,
    pub(super) edits: Vec<InsertEdit>,
    /// The length in bytes of the text `edits` leave inserted
    inserted_len: usize,
}

impl InsertState {
    pub(super) fn record_insert(&mut self, c: char) {
        self.inserted_len += c.len_utf8();
        self.edits.push(InsertEdit::Char(c));
    }

    pub(super) fn record_paste(&mut self, text: &str) {
        self.inserted_len += text.len();
        if self.count.is_some() {
            self.edits.push(InsertEdit::Paste(text.into()));
        }
    }

    pub(super) fn invalidate_autoindent(&mut self, buffer: BufferId) {
        if self.autoindent.is_some_and(|(autoindent_buffer, _)| autoindent_buffer == buffer) {
            self.autoindent = None;
        }
    }

    /// Record a backspace that deleted a grapheme `len` bytes long
    pub(super) fn record_delete(&mut self, len: usize) {
        match self.inserted_len.checked_sub(len) {
            Some(inserted_len) => {
                self.inserted_len = inserted_len;
                self.edits.push(InsertEdit::DeleteChar);
            }
            // Deleting text from before the insert drops the count, as in vim
            None => self.count = None,
        }
    }
}

#[derive(Debug)]
pub(super) enum InsertEdit {
    Char(char),
    Paste(Box<str>),
    DeleteChar,
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

#[derive(Debug)]
pub(super) struct OperatorPendingState {
    pub(crate) operator: Operator,
    /// The count typed before the operator. It multiplies the count typed after it.
    pub(super) count: Option<usize>,
}

impl OperatorPendingState {
    pub(super) fn new(operator: Operator) -> Self {
        Self { operator, count: None }
    }
}
