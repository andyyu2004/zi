use std::ops::RangeBounds;

use super::*;
use crate::editor::Active;
use crate::filetype;

pub struct InspectorBuffer {
    id: BufferId,
    text: String,
    url: Url,
    settings: Settings,
}

impl InspectorBuffer {
    pub fn new(id: BufferId) -> Self {
        Self {
            id,
            url: Url::parse("buffer://inspector").unwrap(),
            settings: Settings {
                readonly: Setting::new(true),
                fix_eol: Setting::new(false),
                ..Default::default()
            },
            text: Default::default(),
        }
    }
}

impl BufferInternal for InspectorBuffer {
    fn id(&self) -> BufferId {
        self.id
    }

    fn flushed(&mut self, _: Internal) {
        panic!("inspector buffer has no backing file")
    }

    fn url(&self) -> &Url {
        &self.url
    }

    fn file_url(&self) -> Option<&Url> {
        None
    }

    fn file_type(&self) -> FileType {
        filetype!(text)
    }

    fn settings(&self) -> &Settings {
        &self.settings
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
        panic!("is readonly")
    }

    fn pre_render(
        &mut self,
        _: Internal,
        client: &Client,
        _view: &crate::db::View,
        _area: tui::Rect,
    ) {
        let buf = self.id;
        client.send(move |editor| {
            if !editor.has_buffer(buf) {
                return Ok(());
            }

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
                        match hl.id.style(&editor.theme().read()) {
                            Some(style) => format!("{capture_name} -> {style}"),
                            None => capture_name.to_string(),
                        }
                    }
                    None => "no highlights".to_string(),
                }
            };

            let this =
                editor.buffer_mut(buf).as_any_mut(Internal(())).downcast_mut::<Self>().unwrap();
            this.text = output;

            Ok(())
        });
    }
}
