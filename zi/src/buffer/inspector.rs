use std::ops::RangeBounds;

use super::*;

pub struct InspectorBuffer {
    id: BufferId,
    text: String,
    path: PathBuf,
    url: Url,
}

impl InspectorBuffer {
    pub fn new(id: BufferId) -> Self {
        Self {
            id,
            path: PathBuf::from("inspector"),
            url: Url::parse(&format!("buffer://zi/inspector")).unwrap(),
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

    fn path(&self) -> &Path {
        &self.path
    }

    fn url(&self) -> &Url {
        &self.url
    }

    fn file_url(&self) -> Option<&Url> {
        None
    }

    fn file_type(&self) -> &FileType {
        &FileType::TEXT
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn edit(&mut self, _delta: &Delta<'_>) {
        panic!("is readonly")
    }

    fn pre_render(&mut self, client: &SyncClient, _view: &View, _area: tui::Rect) {
        let buf = self.id;
        client.request(move |editor| {
            let mut query_cursor = QueryCursor::new();
            let output = {
                let target_view = editor.active_view();
                let target_buffer = editor.buffer(target_view.buffer());
                let cursor = target_view.cursor();
                match target_buffer
                    .syntax_highlights(editor, &mut query_cursor)
                    .find(|hl| hl.range.contains(&cursor))
                {
                    Some(hl) => {
                        let capture_name = target_buffer
                            .syntax()
                            .expect("if buffer has syntax highlights it must have syntax")
                            .highlights_query()
                            .capture_names()[hl.capture_idx as usize];
                        match hl.id.style(editor.theme()) {
                            Some(style) => format!("{capture_name} -> {style}"),
                            None => format!("{capture_name}"),
                        }
                    }
                    None => "no highlights".to_string(),
                }
            };

            let this = editor[buf].as_any_mut().downcast_mut::<InspectorBuffer>().unwrap();
            this.text = output;

            Ok(())
        });
    }
}
