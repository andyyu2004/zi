use std::marker::PhantomData;
use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Nucleo;

use super::*;
use crate::editor::{get, Action};
use crate::text::TextMut;
use crate::{hashmap, trie, Direction, Mode, OpenFlags, ViewId};

pub struct PickerBuffer<P: Picker> {
    id: BufferId,
    /// The view that displays the results
    display_view: ViewId,
    text: Rope,
    nucleo: Nucleo<P::Item>,
    cancel: Cancel,
    keymap: Keymap,
    picker: P,
    url: Url,
}

pub trait Picker: Copy + 'static {
    type Item: Item;

    fn new(preview: ViewId) -> Self;
    fn config(self) -> nucleo::Config;
    fn confirm(self, editor: &mut Editor, item: Self::Item);
    fn select(self, editor: &mut Editor, item: Self::Item);
}

pub struct FilePicker<P> {
    preview: ViewId,
    marker: PhantomData<P>,
}

impl<P> Clone for FilePicker<P> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for FilePicker<P> {}

impl<P> Picker for FilePicker<P>
where
    P: AsRef<Path> + Item,
{
    type Item = P;

    fn new(preview: ViewId) -> Self {
        Self { preview, marker: PhantomData }
    }

    fn config(self) -> nucleo::Config {
        nucleo::Config::DEFAULT.match_paths()
    }

    fn confirm(self, editor: &mut Editor, path: P) {
        let path = path.as_ref();
        assert!(path.is_file(), "directories should not be in the selection");
        // We can close any of the views, they are all in the same group
        editor.close_view(self.preview);
        if let Err(err) = editor.open_active(path) {
            editor.set_error(err);
        }
    }

    fn select(self, editor: &mut Editor, path: P) {
        let path = path.as_ref();
        match editor.open(path, OpenFlags::READONLY) {
            Ok(buffer) => editor.set_buffer(self.preview, buffer),
            Err(err) => editor.set_error(err),
        }
    }
}

impl<P> PickerBuffer<P>
where
    P: Picker,
{
    pub fn new(
        id: BufferId,
        display_view: ViewId,
        notify: impl Fn() + Send + Sync + 'static,
        picker: P,
    ) -> (Self, Injector<P::Item>) {
        let nucleo = Nucleo::new(picker.config(), Arc::new(notify), None, 1);
        let cancel = Cancel::new();
        let injector = Injector::new(nucleo.injector(), cancel.clone());
        (
            Self {
                id,
                display_view,
                cancel,
                nucleo,
                picker,
                url: Url::parse("buffer://zi/picker").unwrap(),
                text: Default::default(),
                keymap: {
                    let next: Action = |editor| Self::select(editor, Direction::Down);
                    let prev: Action = |editor| Self::select(editor, Direction::Up);
                    let confirm: Action = |editor| Self::confirm(editor);
                    let close: Action = |editor| editor.close_active_view();

                    Keymap::from(hashmap! {
                        Mode::Insert => trie! ({
                            "<Esc>" | "<C-c>" => close,
                            "<Tab>" | "<C-j>" => next,
                            "<S-Tab>" | "<C-k>" => prev,
                            "<CR>" => confirm,
                        }),
                        Mode::Normal => trie!({
                            "<Esc>" | "<C-c>" | "q" => close,
                            "<Tab>" | "<C-j>" | "j" => next,
                            "<S-Tab>" | "<C-k>" | "k" => prev,
                            "<CR>" => confirm,
                        }),
                    })
                },
            },
            injector,
        )
    }

    pub fn new_with_items(
        id: BufferId,
        display_view: ViewId,
        items: impl IntoIterator<Item = P::Item>,
        notify: impl Fn() + Send + Sync + 'static,
        picker: P,
    ) -> Self {
        let (this, inj) = Self::new(id, display_view, notify, picker);
        for item in items {
            if inj.push(item).is_err() {
                break;
            }
        }
        this
    }
}

impl<P: Picker> PickerBuffer<P> {
    fn item(&self, line: u32) -> Option<P::Item> {
        self.nucleo.snapshot().get_matched_item(line).map(|item| item.data.clone())
    }

    fn confirm(editor: &mut Editor) {
        let (_, picker_buf) = get!(editor as Self);
        let display_view = picker_buf.display_view;
        let cursor = editor.view(display_view).cursor();

        let (_, picker_buf) = get!(editor as Self);
        let picker = picker_buf.picker;
        if let Some(item) = picker_buf.item(cursor.line().raw()) {
            picker.confirm(editor, item);
        }
    }

    fn select_current(buf_id: BufferId, editor: &mut Editor) {
        let picker_buf = editor.buffer(buf_id).as_any().downcast_ref::<Self>().unwrap();
        let display_view = picker_buf.display_view;
        let cursor = editor.get_cursor(display_view);

        let picker = picker_buf.picker;
        if let Some(item) = picker_buf.item(cursor.line().raw()) {
            picker.select(editor, item);
        }
    }

    fn select(editor: &mut Editor, direction: Direction) {
        assert!(direction.is_vertical());

        let (_, picker_buf) = get!(editor as Self);
        let display_view = picker_buf.display_view;
        let buf_id = picker_buf.id;
        editor.move_cursor(display_view, direction, 1);

        Self::select_current(buf_id, editor);
    }
}

impl<P: Picker> Buffer for PickerBuffer<P> {
    fn id(&self) -> BufferId {
        self.id
    }

    fn flags(&self) -> BufferFlags {
        BufferFlags::empty()
    }

    fn path(&self) -> &Path {
        Path::new("picker")
    }

    fn url(&self) -> &Url {
        &self.url
    }

    fn file_url(&self) -> Option<&Url> {
        None
    }

    fn file_type(&self) -> &FileType {
        &FileType::PICKER
    }

    fn tab_width(&self) -> u8 {
        4
    }

    fn text(&self) -> &dyn AnyText {
        &self.text
    }

    fn version(&self) -> u32 {
        0
    }

    fn edit(&mut self, delta: &Delta<'_>) {
        self.text.edit(delta);

        let search = self.text.to_string();
        tracing::debug!(%search, "update picker search pattern");
        self.nucleo.pattern.reparse(0, &search, CaseMatching::Smart, Normalization::Smart, false);
    }

    fn pre_render(&mut self, sender: &SyncClient, _view: &View, _area: tui::Rect) {
        self.nucleo.tick(10);

        let snapshot = self.nucleo.snapshot();
        let items = snapshot
            .matched_items(..snapshot.matched_item_count().min(100))
            .map(|item| item.data.clone())
            .collect::<Vec<_>>();

        let display_view = self.display_view;
        let buf_id = self.id;
        sender.request(move |editor| {
            // call `select` on the current line as the set of items may have changed.
            Self::select_current(buf_id, editor);

            // update the display view with the new items
            use std::fmt::Write;
            let buf = editor.view(display_view).buffer();
            let text = editor.buffer(buf).text();

            let mut s = String::new();
            for item in items.iter() {
                writeln!(s, "{item}")?;
            }

            editor.edit(display_view, &Delta::new(0..text.len_bytes(), s));
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
