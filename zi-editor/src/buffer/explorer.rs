use std::marker::PhantomData;
use std::path::MAIN_SEPARATOR;
use std::sync::Arc;

use nucleo::Nucleo;

use super::*;
use crate::editor::{get, Action, Backend};
use crate::syntax::HighlightName;
use crate::{filetype, hashmap, trie, Mode};

pub struct ExplorerBuffer<T: Entry, F: 'static, B> {
    id: BufferId,
    url: Url,
    text: String,
    nucleo: Nucleo<T>,
    cancel: Cancel,
    keymap: Keymap,
    confirm: F,
    config: Settings,
    _backend: PhantomData<fn() -> B>,
}

impl<T, F, B> ExplorerBuffer<T, F, B>
where
    T: Entry,
    F: Fn(&mut Editor<B>, T) + Copy,
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
        let keymap = {
            let noop: Action<B> = |_| {};
            let confirm: Action<B> = |editor| {
                let (view, buf) = get!(editor as Self);
                let cursor = view.cursor();
                if let Some(data) = buf
                    .nucleo
                    .snapshot()
                    .get_matched_item(cursor.line() as u32)
                    .map(|item| item.data.clone())
                {
                    let confirm = buf.confirm;
                    confirm(editor, data);
                }
            };
            Keymap::from(hashmap! {
                Mode::Normal => trie! ({
                    // Prevent the user from using insert mode in this buffer
                    "i" => noop,
                    "<CR>" => confirm,
                }),
            })
        };
        (
            Self {
                id,
                nucleo,
                cancel,
                keymap,
                confirm,
                url: Url::parse("buffer://explorer").unwrap(),
                config: Default::default(),
                text: Default::default(),
                _backend: PhantomData,
            },
            injector,
        )
    }
}

impl<T: Entry, F: Send + Sync, B: Backend> BufferInternal<B> for ExplorerBuffer<T, F, B> {
    fn id(&self) -> BufferId {
        self.id
    }

    fn flags(&self) -> BufferFlags {
        BufferFlags::READONLY
    }

    fn flushed(&mut self, _: Internal) {
        unreachable!("explorer buffer does not have a backing file")
    }

    // fn path(&self) -> &Path {
    //     // TODO get the actual directory path we're looking at
    //     Path::new("explorer")
    // }

    fn url(&self) -> &Url {
        &self.url
    }

    fn file_url(&self) -> Option<&Url> {
        None
    }

    fn file_type(&self) -> FileType {
        filetype!(explorer)
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self, _: Internal) -> &mut dyn Any {
        self
    }

    fn edit_flags(&mut self, _: Internal, _deltas: &Deltas<'_>, _flags: EditFlags) {
        unreachable!("explorer buffer is read-only")
    }

    fn keymap(&mut self, _: Internal) -> Option<&mut Keymap> {
        Some(&mut self.keymap)
    }

    fn pre_render(&mut self, _: Internal, _client: &Client<B>, _view: &View, area: tui::Rect) {
        if !self.nucleo.tick(10).changed {
            return;
        }

        let snapshot = self.nucleo.snapshot();
        self.text.clear();
        let n = snapshot.matched_item_count().min(area.height as u32);
        for item in snapshot.matched_items(..n) {
            self.text.push_str(&format!("{}\n", item.data));
        }
    }

    fn overlay_highlights<'a>(
        &'a self,
        editor: &'a Editor<B>,
        _view: &View,
        _size: Size,
    ) -> Box<dyn Iterator<Item = Highlight> + 'a> {
        Box::new(
            self.text.lines().enumerate().filter(|(_i, line)| line.ends_with(MAIN_SEPARATOR)).map(
                |(i, line)| Highlight {
                    range: PointRange::new((i, 0usize), (i, line.len())),
                    id: editor.highlight_id_by_name(HighlightName::DIRECTORY),
                },
            ),
        )
    }

    fn on_leave(&mut self, _: Internal) {
        self.cancel.cancel();
    }
}
