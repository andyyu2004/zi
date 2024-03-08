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
    select: G,
}

impl<T, F> PickerBuffer<T, F>
where
    T: Item,
    F: Fn(&mut Editor, T) + Copy + 'static,
{
    pub fn new(
        id: BufferId,
        config: nucleo::Config,
        notify: impl Fn() + Send + Sync + 'static,
        confirm: F,
    ) -> (Self, Injector<T>) {
        Self::new_with_select(id, config, notify, confirm, |_, _| ())
    }

    pub fn new_with_items(
        id: BufferId,
        config: nucleo::Config,
        items: impl IntoIterator<Item = T>,
        notify: impl Fn() + Send + Sync + 'static,
        confirm: F,
    ) -> Self {
        let (this, inj) = Self::new(id, config, notify, confirm);
        for item in items {
            let _ = inj.push(item);
        }
        this
    }
}

impl<T, F, G> PickerBuffer<T, F, G>
where
    T: Item,
    F: Fn(&mut Editor, T) + Copy + 'static,
    G: Fn(&mut Editor, T) + Copy + 'static,
{
    pub fn new_with_select(
        id: BufferId,
        config: nucleo::Config,
        notify: impl Fn() + Send + Sync + 'static,
        confirm: F,
        select: G,
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
                select,
                text: Rope::new(),
                selected_line: Line::default(),
                keymap: {
                    let next: Action = |editor| {
                        let (_, buf) = active!(editor as Self);

                        if let Some(item) = buf.select_next() {
                            let select = buf.select;
                            select(editor, item);
                        }
                    };
                    let prev: Action = |editor| {
                        let (_, buf) = active!(editor as Self);

                        if let Some(item) = buf.select_prev() {
                            let select = buf.select;
                            select(editor, item);
                        }
                    };
                    let confirm: Action = |editor| {
                        let (_, buf) = active!(editor as Self);
                        if let Some(item) = buf.selected_item() {
                            let confirm = buf.confirm;
                            confirm(editor, item);
                        }
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

    fn selected_item(&self) -> Option<T> {
        self.nucleo
            .snapshot()
            .get_matched_item(self.selected_line.idx() as u32)
            .map(|item| item.data)
            .cloned()
    }

    fn select_next(&mut self) -> Option<T> {
        if self.selected_line.raw() < self.nucleo.snapshot().matched_item_count().saturating_sub(1)
        {
            self.selected_line += 1
        }

        self.selected_item()
    }

    fn select_prev(&mut self) -> Option<T> {
        if self.selected_line.raw() > 0 {
            self.selected_line -= 1
        }
        self.selected_item()
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

    fn text(&self) -> &dyn Text {
        &self.text
    }

    fn version(&self) -> u32 {
        0
    }

    fn insert_char(&mut self, _pos: Position, c: char) {
        // TODO respect position
        self.text.insert_char(self.end_char_idx, c);
        self.end_char_idx += 1;
        self.selected_line = Line::from(0);

        let search = Cow::from(self.text.slice(..self.end_char_idx));
        tracing::debug!(%search, "update picker search pattern");
        self.nucleo.pattern.reparse(0, &search, CaseMatching::Smart, Normalization::Smart, false);
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
        self.text = Rope::from(self.text.slice(..self.end_char_idx));

        // the number of items that will fit on the screen
        let limit = area.height.saturating_sub(self.text.len_lines() as u16) as u32;

        let offset =
            snapshot.matched_item_count().min(self.selected_line.raw().saturating_sub(limit));

        let n = snapshot.matched_item_count().min(1 + limit + offset);

        for item in snapshot.matched_items(offset..n) {
            self.text.insert(self.text.len_chars(), &format!("\n{}", item.data));
        }
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

impl<T, F, G> fmt::Display for PickerBuffer<T, F, G>
where
    T: Item,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, line) in self.text.lines().enumerate() {
            if i == self.selected_line.raw() as usize {
                write!(f, ">{line}")?;
            } else {
                write!(f, "{line}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use slotmap::KeyData;

    use super::*;
    use crate::ViewId;

    #[test]
    fn picker_smoke() {
        let paths = [
            "zi/src/buffer/tests.rs",
            "zi/src/buffer.rs",
            "zi/src/editor/tests.rs",
            "zi/src/editor.rs",
        ];
        assert!(paths.is_sorted_by(|a, b| Path::new(a) <= Path::new(b)));

        let mut picker = PickerBuffer::new_with_items(
            BufferId::from(KeyData::from_ffi(0)),
            nucleo::Config::DEFAULT.match_paths(),
            paths,
            || {},
            |_, _| (),
        );

        let view = View::new(ViewId::from(KeyData::from_ffi(0)), picker.id());
        let area = tui::Rect::new(5, 10, 20, 5);

        picker.pre_render(&view, tui::Rect::new(5, 10, 20, 5));
        assert_eq!(picker.text.len_lines(), 5);
        assert_eq!(
            picker.selected_item(),
            Some("zi/src/buffer/tests.rs"),
            "first item should be auto selected"
        );

        picker.insert_char(Position::default(), 'z');
        picker.insert_char(Position::default(), 'i');
        picker.pre_render(&view, area);

        expect![[r#"
            >zi
            zi/src/buffer.rs
            zi/src/editor.rs
            zi/src/buffer/tests.rs
            zi/src/editor/tests.rs"#]]
        .assert_eq(&picker.to_string());

        assert_eq!(picker.selected_item(), Some("zi/src/buffer.rs"));
        picker.insert_char(Position::default(), '/');
        picker.insert_char(Position::default(), 'b');
        picker.pre_render(&view, area);

        expect![[r#"
            >zi/b
            zi/src/buffer.rs
            zi/src/buffer/tests.rs"#]]
        .assert_eq(&picker.to_string());
        assert_eq!(picker.selected_item(), Some("zi/src/buffer.rs"));

        assert_eq!(picker.select_next(), Some("zi/src/buffer/tests.rs"));
        assert_eq!(picker.select_next(), Some("zi/src/buffer/tests.rs"));
        assert_eq!(picker.select_prev(), Some("zi/src/buffer.rs"));
        assert_eq!(picker.select_prev(), Some("zi/src/buffer.rs"));

        assert_eq!(picker.select_next(), Some("zi/src/buffer/tests.rs"));

        picker.insert_char(Position::default(), '/');
        picker.insert_char(Position::default(), 't');
        picker.pre_render(&view, area);

        assert_eq!(picker.select_next(), Some("zi/src/buffer/tests.rs"));
    }

    #[test]
    fn picker_scroll() {
        let paths = [
            "zi/src/buffer/tests.rs",
            "zi/src/buffer.rs",
            "zi/src/editor/tests.rs",
            "zi/src/editor.rs",
            "zi/src/syntax.rs",
            "zi/src/view.rs",
        ];
        assert!(paths.is_sorted_by(|a, b| Path::new(a) <= Path::new(b)));

        let mut picker = PickerBuffer::new_with_items(
            BufferId::from(KeyData::from_ffi(0)),
            nucleo::Config::DEFAULT.match_paths(),
            paths,
            || {},
            |_, _| (),
        );

        let height = 4;
        let view = View::new(ViewId::from(KeyData::from_ffi(0)), picker.id());
        let area = tui::Rect::new(5, 10, 20, height);

        picker.pre_render(&view, area);

        expect![[r#"
            >
            zi/src/buffer/tests.rs
            zi/src/buffer.rs
            zi/src/editor/tests.rs
            zi/src/editor.rs"#]]
        .assert_eq(&picker.to_string());

    }
}
