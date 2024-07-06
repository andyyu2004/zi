use std::marker::PhantomData;
use std::mem;
use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Nucleo;
use zi_text::TextMut;

use super::*;
use crate::editor::{get, Action};
use crate::{
    filetype, hashmap, trie, Active, Direction, Mode, OpenFlags, VerticalAlignment, ViewId,
};

pub struct PickerBuffer<P: Picker> {
    id: BufferId,
    /// The view that displays the results
    display_view: ViewId,
    text: String,
    nucleo: Nucleo<P::Entry>,
    cancel: Cancel,
    keymap: Keymap,
    picker: P,
    url: Url,
    dynamic_handler: Option<DynamicHandler<P::Entry>>,
    config: Settings,
}

pub type DynamicHandler<T> = Arc<dyn Fn(Injector<T>, &str) + Send + Sync>;

pub trait Picker: Send + Sync + Copy + 'static {
    type Entry: Entry;

    fn new(preview: ViewId) -> Self;

    fn select(self, editor: &mut Editor, entry: Self::Entry);

    fn confirm(self, editor: &mut Editor, entry: Self::Entry);

    fn config(self) -> nucleo::Config {
        nucleo::Config::DEFAULT
    }
}

pub trait BufferPickerEntry: Entry {
    /// Return an open buffer id or a path to open
    fn buffer_or_path(&self) -> Result<BufferId, &Path>;

    fn point(&self) -> Option<Point>;
}

impl<P> BufferPickerEntry for P
where
    P: AsRef<Path> + Entry,
{
    #[inline]
    fn buffer_or_path(&self) -> Result<BufferId, &Path> {
        Err(self.as_ref())
    }

    #[inline]
    fn point(&self) -> Option<Point> {
        None
    }
}

pub struct BufferPicker<P> {
    preview: ViewId,
    marker: PhantomData<P>,
}

impl<P> Clone for BufferPicker<P> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for BufferPicker<P> {}

impl<P> Picker for BufferPicker<P>
where
    P: BufferPickerEntry,
{
    type Entry = P;

    fn new(preview: ViewId) -> Self {
        Self { preview, marker: PhantomData }
    }

    fn config(self) -> nucleo::Config {
        nucleo::Config::DEFAULT.match_paths()
    }

    fn select(self, editor: &mut Editor, entry: Self::Entry) {
        let point = entry.point();

        let preview = move |editor: &mut Editor, buf: BufferId| {
            editor.set_buffer(self.preview, buf);
            if let Some(point) = point {
                editor.reveal(self.preview, point, VerticalAlignment::Center)
            }
        };

        let path = match entry.buffer_or_path() {
            Ok(buffer) => return preview(editor, buffer),
            Err(path) => path,
        };

        let fut = match editor.open(path, OpenFlags::READONLY | OpenFlags::BACKGROUND) {
            Ok(fut) => fut,
            Err(err) if err.kind() == std::io::ErrorKind::InvalidData => {
                // Probably due to non-utf8 data, show an empty buffer
                editor.set_buffer(self.preview, editor.empty_buffer());
                return;
            }
            Err(err) => {
                editor.set_error(err);
                return;
            }
        };

        editor.callback("open preview", async move { Ok(fut.await?) }, move |editor, buf| {
            preview(editor, buf);
            Ok(())
        });
    }

    fn confirm(self, editor: &mut Editor, entry: Self::Entry) {
        let point = entry.point();

        let path = match entry.buffer_or_path() {
            Ok(buffer) => {
                editor.close_view(self.preview);
                editor.set_buffer(Active, buffer);
                if let Some(point) = point {
                    editor.reveal(Active, point, VerticalAlignment::Center);
                }
                return;
            }
            Err(path) => path,
        };

        assert!(path.is_file(), "directories should not be in the selection");

        // We can close any of the views, they are all in the same group
        editor.close_view(self.preview);

        let fut = editor.open(path, OpenFlags::SPAWN_LANGUAGE_SERVERS);
        editor.callback("confirm selection", async move { Ok(fut?.await?) }, move |editor, _buf| {
            if let Some(point) = entry.point() {
                editor.reveal(Active, point, VerticalAlignment::Center);
            }
            Ok(())
        })
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
    ) -> Self {
        let nucleo = Nucleo::new(picker.config(), Arc::new(notify), None, 1);
        let cancel = Cancel::new();
        Self {
            id,
            display_view,
            cancel,
            nucleo,
            picker,
            dynamic_handler: None,
            url: Url::parse("buffer://picker").unwrap(),
            config: Default::default(),
            text: Default::default(),
            keymap: {
                let next: Action = |editor| Self::select(editor, Direction::Down);
                let prev: Action = |editor| Self::select(editor, Direction::Up);
                let confirm: Action = |editor| Self::confirm(editor);
                let close: Action = |editor| editor.close_view(Active);

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
        }
    }

    pub fn new_with_items(
        id: BufferId,
        display_view: ViewId,
        items: impl IntoIterator<Item = P::Entry>,
        notify: impl Fn() + Send + Sync + 'static,
        picker: P,
    ) -> Self {
        let this = Self::new(id, display_view, notify, picker);
        let inj = this.injector();
        for item in items {
            if inj.push(item).is_err() {
                break;
            }
        }
        this
    }

    pub fn injector(&self) -> Injector<P::Entry> {
        Injector::new(self.nucleo.injector(), self.cancel.clone())
    }

    #[must_use]
    pub fn with_dynamic_handler(mut self, handler: DynamicHandler<P::Entry>) -> Self {
        self.dynamic_handler = Some(handler);
        self
    }
}

impl<P: Picker> PickerBuffer<P> {
    fn item(&self, line: u32) -> Option<P::Entry> {
        self.nucleo.snapshot().get_matched_item(line).map(|item| item.data.clone())
    }

    fn confirm(editor: &mut Editor) {
        let (_, picker_buf) = get!(editor as Self);
        let display_view = picker_buf.display_view;
        let cursor = editor.view(display_view).cursor();

        let (_, picker_buf) = get!(editor as Self);
        let picker = picker_buf.picker;
        if let Some(item) = picker_buf.item(cursor.line() as u32) {
            picker.confirm(editor, item);
        }
    }

    fn select_current(buf_id: BufferId, editor: &mut Editor) {
        let picker_buf = editor[buf_id].as_any().downcast_ref::<Self>().unwrap();
        let display_view = picker_buf.display_view;
        let cursor = editor.cursor(display_view);

        let picker = picker_buf.picker;
        if let Some(item) = picker_buf.item(cursor.line() as u32) {
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

impl<P: Picker + Send + Sync> BufferInternal for PickerBuffer<P> {
    fn id(&self) -> BufferId {
        self.id
    }

    fn flags(&self) -> BufferFlags {
        BufferFlags::empty()
    }

    fn flushed(&mut self, _: Internal) {
        panic!("picker buffer has no backing file")
    }

    fn url(&self) -> &Url {
        &self.url
    }

    fn file_url(&self) -> Option<&Url> {
        None
    }

    fn file_type(&self) -> FileType {
        filetype!(picker)
    }

    fn settings(&self) -> &Settings {
        &self.config
    }

    fn text(&self) -> &(dyn AnyText + 'static) {
        &self.text
    }

    fn version(&self) -> u32 {
        0
    }

    fn edit_flags(&mut self, _: Internal, deltas: &Deltas<'_>, _flags: EditFlags) {
        self.text.edit(deltas);

        tracing::debug!(%self.text, "update picker search pattern");

        if let Some(handler) = &self.dynamic_handler {
            // swap the cancel token with a fresh one and cancel the previous one
            let cancel = mem::take(&mut self.cancel);
            cancel.cancel();
            self.nucleo.restart(false);
            // if there is a dynamic handler, delegate the updated prompt to it
            handler(self.injector(), &self.text);
        } else {
            self.nucleo.pattern.reparse(
                0,
                &self.text,
                CaseMatching::Smart,
                Normalization::Smart,
                false,
            );
        }
    }

    fn pre_render(&mut self, _: Internal, client: &Client, _view: &View, _area: tui::Rect) {
        if !self.nucleo.tick(10).changed {
            return;
        }

        let snapshot = self.nucleo.snapshot();
        let items = snapshot
            .matched_items(..snapshot.matched_item_count().min(100))
            .map(|item| item.data.clone())
            .collect::<Vec<_>>();

        let display_view = self.display_view;
        let buf_id = self.id;
        client.send(move |editor| {
            // call `select` on the current line as the set of items may have changed.
            Self::select_current(buf_id, editor);

            // update the display view with the new items
            use std::fmt::Write;
            let buf = editor.view(display_view).buffer();
            let text = editor[buf].text();

            let mut s = String::new();
            for item in items.iter() {
                writeln!(s, "{item}")?;
            }

            editor.edit(display_view, &Deltas::new([Delta::new(0..text.len_bytes(), s)]))?;
            Ok(())
        });
    }

    fn keymap(&mut self, _: Internal) -> Option<&mut Keymap> {
        Some(&mut self.keymap)
    }

    fn on_leave(&mut self, _: Internal) {
        self.cancel.cancel()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self, _: Internal) -> &mut dyn Any {
        self
    }
}
