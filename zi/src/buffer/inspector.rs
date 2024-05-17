use std::ops::RangeBounds;

use super::*;
use crate::editor::Active;

pub struct InspectorBuffer {
    id: BufferId,
    text: String,
    path: PathBuf,
    url: Url,
    config: Config,
}

impl InspectorBuffer {
    pub fn new(id: BufferId) -> Self {
        Self {
            id,
            path: PathBuf::from("inspector"),
            url: Url::parse("buffer://zi/inspector").unwrap(),
            config: Default::default(),
            text: Default::default(),
        }
    }
}

impl Buffer for InspectorBuffer {
    fn id(&self) -> BufferId {
        self.id
    }

    fn flags(&self) -> BufferFlags {
        BufferFlags::READONLY
    }

    fn flushed(&mut self, _: Internal) {
        panic!("inspector buffer has no backing file")
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn url(&self) -> &Url {
        &self.url
    }

    fn file_url(&self) -> Option<&Url> {
        None
    }

    fn file_type(&self) -> FileType {
        FileType::TEXT
    }

    fn config(&self) -> &Config {
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

    fn edit(&mut self, _: Internal, _deltas: &Deltas<'_>) {
        panic!("is readonly")
    }

    fn pre_render(&mut self, _: Internal, client: &SyncClient, _view: &View, _area: tui::Rect) {
        let buf = self.id;
        client.request(move |editor| {
            let mut query_cursor = QueryCursor::new();
            let output = {
                let target_view = editor.view(Active);
                let target_buffer = editor.buffer(target_view);
                let cursor = target_view.cursor();
                match target_buffer
                    .syntax_highlights(
                        editor,
                        &mut query_cursor,
                        PointRange::new((0usize, 0usize), (usize::MAX, usize::MAX)),
                    )
                    .find(|hl| hl.range.contains(&cursor))
                {
                    Some(hl) => {
                        let capture_name = target_buffer
                            .syntax()
                            .expect("if buffer has syntax highlights it must have syntax")
                            .capture_index_to_name(hl.capture_idx);
                        match hl.id.style(editor.theme()) {
                            Some(style) => format!("{capture_name} -> {style}"),
                            None => capture_name.to_string(),
                        }
                    }
                    None => "no highlights".to_string(),
                }
            };

            let this = editor
                .buffer_mut(buf)
                .as_any_mut(Internal(()))
                .downcast_mut::<InspectorBuffer>()
                .unwrap();
            this.text = output;

            Ok(())
        });
    }
}
