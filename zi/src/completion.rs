use std::cell::RefCell;

use futures_core::future::BoxFuture;
use nucleo::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo::Utf32Str;
use zi_core::{CompletionItem, Point};
use zi_text::{Delta, DeltaRange};

use crate::{BufferId, Editor};

pub trait CompletionProvider: Send + Sync + 'static {
    fn completions(
        &self,
        editor: &mut Editor,
        params: CompletionParams,
    ) -> BoxFuture<'static, zi_lsp::Result<Vec<CompletionItem>>>;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CompletionParams {
    pub buf: BufferId,
    pub point: Point,
}

#[derive(Debug, Default)]
pub(crate) enum Completion {
    Active(ActiveCompletionState),
    #[default]
    Inactive,
}

#[derive(Debug, Default)]
pub(crate) struct ActiveCompletionState {
    pub widget_state: RefCell<tui::ListState>,
    replacement_range: DeltaRange,
    options: Vec<CompletionItem>,
    matches: Vec<nucleo::Match>,
    matcher: nucleo::Matcher,
    query: String,
}

impl Completion {
    pub(super) fn deactivate(&mut self) {
        *self = Completion::Inactive;
    }

    pub(super) fn activate(&mut self, trigger: Option<char>) {
        if let Completion::Inactive = self {
            let query = match trigger {
                Some(c) if c.is_alphanumeric() => c.to_string(),
                _ => String::new(),
            };
            *self = Completion::Active(ActiveCompletionState { query, ..Default::default() });
        }
    }

    pub(super) fn matches(&self) -> impl ExactSizeIterator<Item = &CompletionItem> {
        match self {
            Completion::Inactive => {
                Box::new([].iter()) as Box<dyn ExactSizeIterator<Item = &CompletionItem>>
            }
            Completion::Active(state) => Box::new(state.matches()),
        }
    }
}

impl ActiveCompletionState {
    pub fn select(&mut self) -> Option<Delta<'_>> {
        let item = self
            .widget_state
            .borrow()
            .selected()
            .and_then(|idx| self.matches.get(idx))
            .and_then(|m| self.options.get(m.idx as usize))?;

        let replacement_text = item.insert_text.as_deref().unwrap_or(&item.label);
        let n = replacement_text.len();
        let delta = Delta::new(self.replacement_range.clone(), replacement_text);
        self.replacement_range.end = self.replacement_range.start + n;
        Some(delta)
    }

    pub fn start_byte(&self) -> usize {
        self.replacement_range.start
    }

    pub fn set_items(&mut self, start_byte: usize, options: Vec<CompletionItem>) {
        self.options = options;
        self.replacement_range = start_byte..start_byte;
        self.query.clear();
        self.compute_matches();
    }

    pub fn update_query(&mut self, c: Option<char>) {
        match c {
            Some(c) => {
                self.query.push(c);
                self.replacement_range.end = self.replacement_range.start + self.query.len();
            }
            None => {
                if self.query.pop().is_some() {
                    self.replacement_range.end = self.replacement_range.start + self.query.len();
                }
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
        tracing::debug!(query = self.query, "completion matches");
        let pattern = Atom::new(
            &self.query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
            false,
        );

        let mut buf = vec![];
        self.matches.clear();
        self.matches.extend(self.options.iter().enumerate().filter_map(|(idx, item)| {
            pattern
                .score(
                    Utf32Str::new(item.filter_text.as_ref().unwrap_or(&item.label), &mut buf),
                    &mut self.matcher,
                )
                .map(|score| nucleo::Match { idx: idx as u32, score: score as u32 })
        }));
        self.matches.sort_by(|a, b| b.score.cmp(&a.score));

        self.widget_state.borrow_mut().select(None);
    }
}
