use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Nucleo;

use super::*;
use crate::editor::{get, Action};
use crate::{hashmap, trie, Direction, Editor, Mode, ViewId};

pub struct PickerBuffer<T: Item, F, G = fn(&mut Editor, T)> {
    id: BufferId,
    /// The view that displays the results
    display_view: ViewId,
    text: Rope,
    nucleo: Nucleo<T>,
    cancel: Cancel,
    keymap: Keymap,
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
        display_view: ViewId,
        config: nucleo::Config,
        notify: impl Fn() + Send + Sync + 'static,
        confirm: F,
    ) -> (Self, Injector<T>) {
        Self::new_with_select(id, display_view, config, notify, confirm, |_, _| ())
    }

    pub fn new_with_items(
        id: BufferId,
        display_view: ViewId,
        config: nucleo::Config,
        items: impl IntoIterator<Item = T>,
        notify: impl Fn() + Send + Sync + 'static,
        confirm: F,
    ) -> Self {
        let (this, inj) = Self::new(id, display_view, config, notify, confirm);
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
        display_view: ViewId,
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
                display_view,
                cancel,
                nucleo,
                confirm,
                select,
                text: Rope::new(),
                keymap: {
                    let next: Action = |editor| Self::select(editor, Direction::Down);
                    let prev: Action = |editor| Self::select(editor, Direction::Up);
                    let confirm: Action = |editor| Self::confirm(editor);

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
            },
            injector,
        )
    }
}

impl<T: Item, F: Fn(&mut Editor, T) + Copy + 'static, G: Fn(&mut Editor, T) + Copy + 'static>
    PickerBuffer<T, F, G>
{
    fn item(&self, line: u32) -> T {
        self.nucleo.snapshot().get_matched_item(line).expect("should be in bounds").data.clone()
    }

    fn confirm(editor: &mut Editor) {
        let (_, picker_buf) = get!(editor as Self);
        let display_view = picker_buf.display_view;
        let cursor = editor.view(display_view).cursor();

        let (_, picker_buf) = get!(editor as Self);
        let item = picker_buf.item(cursor.line().raw());
        let confirm = picker_buf.confirm;
        confirm(editor, item);
    }

    fn select(editor: &mut Editor, direction: Direction) {
        assert!(direction.is_vertical());

        let (_, picker_buf) = get!(editor as Self);
        let display_view = picker_buf.display_view;
        let cursor = editor.move_cursor(display_view, direction, 1);

        let (_, picker_buf) = get!(editor as Self);
        let item = picker_buf.item(cursor.line().raw());
        let select = picker_buf.select;
        select(editor, item);
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

    fn edit(&mut self, change: &Change<'_>) {
        self.text.edit(change);

        let search = Cow::from(&self.text);
        tracing::debug!(%search, "update picker search pattern");
        self.nucleo.pattern.reparse(0, &search, CaseMatching::Smart, Normalization::Smart, false);
    }

    fn pre_render(&mut self, sender: &TaskSender, _view: &View, _area: tui::Rect) {
        self.nucleo.tick(10);

        let snapshot = self.nucleo.snapshot();
        let items = snapshot
            .matched_items(..snapshot.matched_item_count().min(100))
            .map(|item| item.data.clone())
            .collect::<Vec<_>>();
        let display_view = self.display_view;
        sender.queue(move |editor| {
            // hacks hacks hacks need a better interface to modify buffers first
            let mut ops = smallvec::smallvec![Operation::Clear];
            for item in items {
                ops.push(Operation::Append(item.to_string().into()));
                ops.push(Operation::Append("\n".into()));
            }
            editor.edit(display_view, &Change::new(ops));
            Ok(())
        });
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

#[cfg(test)]
mod tests {
    // use expect_test::expect;
    // use slotmap::KeyData;
    //
    // use super::*;
    // use crate::ViewId;

    #[test]
    fn picker_smoke() {
        // let paths = [
        //     "zi/src/buffer/tests.rs",
        //     "zi/src/buffer.rs",
        //     "zi/src/editor/tests.rs",
        //     "zi/src/editor.rs",
        // ];
        // assert!(paths.is_sorted_by(|a, b| Path::new(a) <= Path::new(b)));
        //
        // let mut picker = PickerBuffer::new_with_items(
        //     BufferId::from(KeyData::from_ffi(0)),
        //     nucleo::Config::DEFAULT.match_paths(),
        //     paths,
        //     || {},
        //     |_, _| (),
        // );
        //
        // let view = View::new(ViewId::from(KeyData::from_ffi(0)), picker.id());
        // let area = tui::Rect::new(5, 10, 20, 5);
        //
        // picker.pre_render(&view, tui::Rect::new(5, 10, 20, 5));
        // assert_eq!(picker.text.len_lines(), 5);
        // assert_eq!(
        //     picker.selected_item(),
        //     Some("zi/src/buffer/tests.rs"),
        //     "first item should be auto selected"
        // );
        //
        // picker.insert_char(Position::default(), 'z');
        // picker.insert_char(Position::default(), 'i');
        // picker.pre_render(&view, area);
        //
        // expect![[r#"
        //     >zi
        //     zi/src/buffer.rs
        //     zi/src/editor.rs
        //     zi/src/buffer/tests.rs
        //     zi/src/editor/tests.rs"#]]
        // .assert_eq(&picker.to_string());
        //
        // assert_eq!(picker.selected_item(), Some("zi/src/buffer.rs"));
        // picker.insert_char(Position::default(), '/');
        // picker.insert_char(Position::default(), 'b');
        // picker.pre_render(&view, area);
        //
        // expect![[r#"
        //     >zi/b
        //     zi/src/buffer.rs
        //     zi/src/buffer/tests.rs"#]]
        // .assert_eq(&picker.to_string());
        // assert_eq!(picker.selected_item(), Some("zi/src/buffer.rs"));
        //
        // assert_eq!(picker.select_next(), Some("zi/src/buffer/tests.rs"));
        // assert_eq!(picker.select_next(), Some("zi/src/buffer/tests.rs"));
        // assert_eq!(picker.select_prev(), Some("zi/src/buffer.rs"));
        // assert_eq!(picker.select_prev(), Some("zi/src/buffer.rs"));
        //
        // assert_eq!(picker.select_next(), Some("zi/src/buffer/tests.rs"));
        //
        // picker.insert_char(Position::default(), '/');
        // picker.insert_char(Position::default(), 't');
        // picker.pre_render(&view, area);
        //
        // assert_eq!(picker.select_next(), Some("zi/src/buffer/tests.rs"));
    }

    #[test]
    fn picker_scroll() {
        // let paths = [
        //     "zi/src/buffer/tests.rs",
        //     "zi/src/buffer.rs",
        //     "zi/src/editor/tests.rs",
        //     "zi/src/editor.rs",
        //     "zi/src/syntax.rs",
        //     "zi/src/view.rs",
        // ];
        // assert!(paths.is_sorted_by(|a, b| Path::new(a) <= Path::new(b)));
        //
        // let mut picker = PickerBuffer::new_with_items(
        //     BufferId::from(KeyData::from_ffi(0)),
        //     nucleo::Config::DEFAULT.match_paths(),
        //     paths,
        //     || {},
        //     |_, _| (),
        // );
        //
        // let height = 4;
        // let view = View::new(ViewId::from(KeyData::from_ffi(0)), picker.id());
        // let area = tui::Rect::new(5, 10, 20, height);
        //
        // picker.pre_render(&view, area);
        //
        // expect![">"].assert_eq(&picker.to_string());
    }
}
