use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Nucleo;

use super::*;
use crate::editor::{active, Action};
use crate::{hashmap, trie, Editor, Line, Mode};

pub struct PickerBuffer<T: Item, F: 'static> {
    id: BufferId,
    text: Rope,
    nucleo: Nucleo<T>,
    end_char_idx: usize,
    cancel: Cancel,
    keymap: Keymap,
    selected_line: Line,
    confirm: F,
    rendered_item_count: u32,
}

impl<T, F> PickerBuffer<T, F>
where
    T: Item,
    F: Fn(&mut Editor, T) + Copy,
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
                text: Rope::new(),
                selected_line: Line::default(),
                rendered_item_count: 0,
                keymap: {
                    let next: Action = |editor| {
                        let (_, buf) = active!(editor as Self);
                        if buf.selected_line.raw() < buf.rendered_item_count - 1 {
                            buf.selected_line += 1
                        }
                    };
                    let prev: Action = |editor| active!(editor as Self).1.selected_line -= 1;
                    let confirm: Action = |editor| {
                        let (_, buf) = active!(editor as Self);
                        let data = buf
                            .nucleo
                            .snapshot()
                            .get_matched_item(buf.selected_line.idx() as u32)
                            .expect("invalid line index")
                            .data
                            .clone();
                        let confirm = buf.confirm;
                        confirm(editor, data);
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
}

impl<T: Item, F: 'static> Buffer for PickerBuffer<T, F> {
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
            Box::new(std::iter::once((
                Range::new((line_idx, 0), (line_idx, size.width as u32)),
                HighlightId(0),
            ))) as Box<dyn Iterator<Item = (Range, HighlightId)>>
        };

        res.unwrap_or_else(|| Box::new(std::iter::empty()))
    }

    fn pre_render(&mut self) {
        self.nucleo.tick(10);
        let snapshot = self.nucleo.snapshot();
        self.text = Rope::from(self.writable_text());
        let mut rope = Rope::new();

        self.rendered_item_count = snapshot.matched_item_count().min(100);
        for item in snapshot.matched_items(..self.rendered_item_count) {
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
