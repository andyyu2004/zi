use super::*;

pub struct TextBuffer<X> {
    id: BufferId,
    path: PathBuf,
    url: Option<Url>,
    text: X,
    language_id: FileType,
    syntax: Option<Syntax>,
    // FIXME highlight map doesn't belong here
    highlight_map: HighlightMap,
    version: u32,
    tab_width: u8,
}

impl<X: Text + 'static> Buffer for TextBuffer<X> {
    fn id(&self) -> BufferId {
        self.id
    }

    fn path(&self) -> &Path {
        &self.path
    }

    #[inline]
    fn url(&self) -> Option<Url> {
        self.url.clone()
    }

    #[inline]
    fn language_id(&self) -> &FileType {
        &self.language_id
    }

    #[inline]
    fn tab_width(&self) -> u8 {
        self.tab_width
    }

    fn text(&self) -> &dyn Text {
        &self.text
    }

    fn insert_char(&mut self, pos: Position, c: char, clear: bool) {
        match self.text.as_text_mut() {
            Some(text) => {
                let idx = text.line_to_char(pos.line().idx()) + pos.col().idx();
                text.insert_char(idx, c, clear);
                if let Some(syntax) = self.syntax.as_mut() {
                    syntax.apply(&self.text);
                }

                self.version.checked_add(1).unwrap();
            }
            // FIXME need to check flags and prevent this
            None => panic!("trying to modify a readonly buffer: {}", std::any::type_name::<X>()),
        }
    }

    fn version(&self) -> u32 {
        self.version
    }

    fn syntax_highlights<'a>(
        &'a self,
        cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + 'a> {
        Box::new(
            self.syntax
                .as_ref()
                .map_or(Highlights::Empty, |syntax| syntax.highlights(cursor, &self.text))
                .map(|capture| (capture.node.range(), self.highlight_map.get(capture.index)))
                .flat_map(move |(range, id)| {
                    // Split multi-line highlights into single-line highlights
                    (range.start_point.row..=range.end_point.row).map(move |idx| {
                        let start =
                            if idx == range.start_point.row { range.start_point.column } else { 0 };
                        let end = if idx == range.end_point.row {
                            range.end_point.column
                        } else {
                            self.text.line(idx).len_chars()
                        };
                        (Range::new(Position::new(idx, start), Position::new(idx, end)), id)
                    })
                }),
        )
    }

    fn overlay_highlights(
        &self,
        view: &View,
        size: Size,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + '_> {
        assert_eq!(view.buffer(), self.id);
        let cursor = view.cursor();
        // The current_line highlight
        Box::new(std::iter::once((
            Range::new(cursor.with_col(0), cursor.with_col(size.width as u32)),
            // FIXME don't use random highlight id
            HighlightId(0),
        )))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl<X: LazyText> TextBuffer<X> {
    #[inline]
    pub fn new(
        id: BufferId,
        language_id: FileType,
        path: impl AsRef<Path>,
        mut text: X,
        theme: &Theme,
    ) -> Self {
        let path = path.as_ref();
        let path = std::fs::canonicalize(path).ok().unwrap_or_else(|| path.to_path_buf());
        let url = Url::from_file_path(&path).ok();

        // ensure the buffer ends with a newline
        if let Some(text) = text.as_text_mut() {
            let idx = text.len_chars();
            if text.get_char(idx.saturating_sub(1)) != Some('\n') {
                text.insert_char(idx, '\n', false);
            }
        }

        let mut syntax = Syntax::for_language(&language_id);
        if let Some(syntax) = &mut syntax {
            syntax.apply(&text);
        }

        Self {
            id,
            path,
            url,
            text,
            language_id,
            version: 0,
            highlight_map: HighlightMap::new(
                syntax.as_ref().map_or(&[][..], |syntax| syntax.highlights_query().capture_names()),
                theme,
            ),
            syntax,
            tab_width: 4,
        }
    }
}
