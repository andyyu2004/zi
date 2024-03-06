use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Nucleo;

use super::*;
use crate::editor::{active, Action};
use crate::{hashmap, trie, Editor, Line, Mode};

pub struct PickerBuffer<T: Item, F, G = fn(&mut Editor, T)> {
    id: BufferId,
    text: Rope,
    nucleo: Nucleo<T>,
    end_char_idx: usize,
    cancel: Cancel,
    keymap: Keymap,
    selected_line: Line,
    confirm: F,
    // It's a bit unnecessarily restrictive to require the same type for `confirm` and `select`
    // We can add another type parameter if we need to
    select: Option<G>,
}

impl<T, F, G> PickerBuffer<T, F, G>
where
    T: Item,
    F: Fn(&mut Editor, T) + Copy + 'static,
    G: Fn(&mut Editor, T) + Copy + 'static,
{
    pub fn new(
        id: BufferId,
        config: nucleo::Config,
        notify: impl Fn() + Send + Sync + 'static,
        confirm: F,
    ) -> (Self, Injector<T>) {
        let nucleo = Nucleo::new(config, Arc::new(notify), None, 1);
        let cancel = Cancel::new();
        let injector = Injector::new(nucleo.injector(), cancel.clone());
        (
            Self {
                id,
                cancel,
                nucleo,
                confirm,
                select: None,
                text: Rope::new(),
                selected_line: Line::default(),
                keymap: {
                    let next: Action = |editor| {
                        let (_, buf) = active!(editor as Self);
                        if buf.selected_line.raw() < buf.nucleo.snapshot().item_count() - 1 {
                            buf.selected_line += 1
                        }

                        if let Some(select) = buf.select {
                            let item = buf.selected_item();
                            select(editor, item);
                        }
                    };
                    let prev: Action = |editor| {
                        let (_, buf) = active!(editor as Self);
                        buf.selected_line -= 1;
                        if let Some(select) = buf.select {
                            let item = buf.selected_item();
                            select(editor, item);
                        }
                    };
                    let confirm: Action = |editor| {
                        let (_, buf) = active!(editor as Self);
                        let item = buf.selected_item();
                        let confirm = buf.confirm;
                        confirm(editor, item);
                    };

                    Keymap::from(hashmap! {
                        Mode::Insert => trie! ({
                            "<Tab>" => next,
                            "<S-Tab>" => prev,
                            "<C-j>" => next,
                            "<C-k>" => prev,
                            "<CR>" => confirm,
                        }),
                        Mode::Normal => trie!({
                            "<Tab>" => next,
                            "<S-Tab>" => prev,
                            "<C-j>" => next,
                            "<C-k>" => prev,
                            "j" => next,
                            "k" => prev,
                            "<CR>" => confirm,
                        }),
                    })
                },
                end_char_idx: 0,
            },
            injector,
        )
    }

    /// Add a callback to be called when the user selects an item
    pub fn with_select(mut self, select: G) -> Self {
        self.select = Some(select);
        self
    }

    fn selected_item(&self) -> T {
        self.nucleo
            .snapshot()
            .get_matched_item(self.selected_line.idx() as u32)
            .expect("invalid line index")
            .data
            .clone()
    }
}

impl<T: Item, F: 'static, G: 'static> Buffer for PickerBuffer<T, F, G> {
    fn id(&self) -> BufferId {
        self.id
    }

    fn path(&self) -> &Path {
        Path::new("picker")
    }

    fn url(&self) -> Option<Url> {
        None
    }

    fn language_id(&self) -> &FileType {
        &FileType::PICKER
    }

    fn tab_width(&self) -> u8 {
        4
    }

    fn text(&self) -> RopeSlice<'_> {
        self.text.slice(..)
    }

    fn version(&self) -> u32 {
        0
    }

    fn insert_char(&mut self, _pos: Position, c: char) {
        // TODO respect position
        self.text.insert_char(self.end_char_idx, c);
        self.end_char_idx += 1;
        self.selected_line = Line::from(0);
        self.nucleo.pattern.reparse(
            0,
            &self.writable_text().to_string(),
            CaseMatching::Smart,
            Normalization::Smart,
            false,
        );
    }

    fn writable_range(&self) -> (Bound<usize>, Bound<usize>) {
        // TODO maintain this correctly
        (Bound::Unbounded, Bound::Excluded(self.end_char_idx))
    }

    fn overlay_highlights(
        &self,
        _view: &View,
        size: Size,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + '_> {
        let res: Option<_> = try {
            let line_idx =
                self.selected_line + self.text.try_char_to_line(self.end_char_idx + 1).ok()?;
            let line_idx = line_idx.raw().min(size.height as u32 - 1);
            let range = Range::new((line_idx, 0), (line_idx, size.width as u32));
            Box::new(std::iter::once((range, HighlightId(0))))
                as Box<dyn Iterator<Item = (Range, HighlightId)>>
        };

        res.unwrap_or_else(|| Box::new(std::iter::empty()))
    }

    fn pre_render(&mut self, _view: &View, area: tui::Rect) {
        self.nucleo.tick(10);
        let snapshot = self.nucleo.snapshot();
        self.text = Rope::from(self.writable_text());
        let mut rope = Rope::new();

        // the number of items that will fit on the screen
        let limit = area.height.saturating_sub(1 + self.writable_text().len_lines() as u16) as u32;

        let offset =
            snapshot.matched_item_count().min(self.selected_line.raw().saturating_sub(limit));

        let n = snapshot.matched_item_count().min(1 + limit + offset);

        for item in snapshot.matched_items(offset..n) {
            rope.insert(rope.len_chars(), &format!("\n{}", item.data));
        }

        self.text.append(rope);
    }

    fn keymap(&mut self) -> Option<&mut Keymap> {
        Some(&mut self.keymap)
    }

    fn on_leave(&mut self) {
        self.cancel.cancel()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
