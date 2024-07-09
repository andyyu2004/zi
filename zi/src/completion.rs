use std::cell::RefCell;
use std::ops::DerefMut;

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
    widget_state: RefCell<tui::ListState>,
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

    pub(super) fn activate(&mut self, at: usize, trigger: Option<char>) {
        if let Completion::Inactive = self {
            let (query, replacement_range) = match trigger {
                Some(c) if c.is_alphabetic() => (c.to_string(), at - c.len_utf8()..at),
                _ => (String::new(), at..at),
            };

            tracing::debug!(initial_query = ?query, trigger = ?trigger, range = ?replacement_range, "activating completion");
            *self = Completion::Active(ActiveCompletionState {
                query,
                replacement_range,
                ..Default::default()
            });
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
    pub fn select_next(&mut self) -> Option<Delta<'static>> {
        let idx = self.widget_state.borrow().selected().map_or(0, |idx| idx + 1);
        if idx >= self.matches.len() {
            return None;
        }

        self.widget_state.borrow_mut().select(Some(idx));
        self.select()
    }

    pub fn select_prev(&mut self) -> Option<Delta<'static>> {
        let mut state = self.widget_state.borrow_mut();
        match state.selected().and_then(|idx| idx.checked_sub(1)) {
            None => {
                state.select(None);
                drop(state);
                Some(self.generate_delta(self.query.to_owned()))
            }
            Some(idx) => {
                state.select(Some(idx));
                drop(state);
                self.select()
            }
        }
    }

    pub fn widget_state(&self) -> impl DerefMut<Target = tui::ListState> + '_ {
        self.widget_state.borrow_mut()
    }

    fn select(&mut self) -> Option<Delta<'static>> {
        let item = self
            .widget_state
            .borrow()
            .selected()
            .and_then(|idx| self.matches.get(idx))
            .and_then(|m| self.options.get(m.idx as usize))?;

        let replacement_text = item.insert_text.as_deref().unwrap_or(&item.label);
        Some(self.generate_delta(replacement_text.to_owned()))
    }

    fn generate_delta<'a>(&mut self, replacement: impl Into<String>) -> Delta<'static> {
        let replacement = replacement.into();
        let n = replacement.len();
        let delta = Delta::new(self.replacement_range.clone(), replacement);
        self.replacement_range.end = self.replacement_range.start + n;
        delta
    }

    pub fn start_byte(&self) -> usize {
        self.replacement_range.start
    }

    pub fn set_items(&mut self, options: Vec<CompletionItem>) {
        self.options = options;
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
        Box::new(self.matches.iter().map(|m| &self.options[m.idx as usize]))
    }

    fn compute_matches(&mut self) {
        tracing::debug!(query = self.query, range = ?self.replacement_range, "completion matches");
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
