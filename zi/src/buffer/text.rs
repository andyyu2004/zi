use super::*;

pub struct TextBuffer {
    id: BufferId,
    path: PathBuf,
    url: Option<Url>,
    text: Rope,
    language_id: FileType,
    syntax: Option<Syntax>,
    // FIXME highlight map doesn't belong here
    highlight_map: HighlightMap,
    version: u32,
    tab_width: u8,
}

impl Buffer for TextBuffer {
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

    fn text(&self) -> RopeSlice<'_> {
        self.text.slice(..)
    }

    fn insert_char(&mut self, pos: Position, c: char) {
        let idx = self.text.line_to_char(pos.line().idx()) + pos.col().idx();
        self.text.insert_char(idx, c);
        if let Some(syntax) = self.syntax.as_mut() {
            syntax.apply(self.text.slice(..));
        }

        self.version.checked_add(1).unwrap();
    }

    fn version(&self) -> u32 {
        self.version
    }

    fn highlights<'a>(
        &'a self,
        cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + 'a> {
        Box::new(
            self.syntax
                .as_ref()
                .map_or(Highlights::Empty, |syntax| syntax.highlights(cursor, self.text.slice(..)))
                .map(|capture| (capture.node.range(), self.highlight_map.get(capture.index))),
        )
    }
}

impl TextBuffer {
    #[inline]
    pub fn new(
        id: BufferId,
        language_id: FileType,
        path: impl AsRef<Path>,
        text: impl Into<Rope>,
        theme: &Theme,
    ) -> Self {
        let path = path.as_ref();
        let path = std::fs::canonicalize(path).ok().unwrap_or_else(|| path.to_path_buf());
        let url = Url::from_file_path(&path).ok();
        let mut text: Rope = text.into();
        let idx = text.len_chars().saturating_sub(1);
        // ensure the buffer ends with a newline
        if text.get_char(idx) != Some('\n') {
            text.insert_char(idx, '\n');
        }

        let mut syntax = Syntax::for_language(&language_id);
        if let Some(syntax) = &mut syntax {
            syntax.apply(text.slice(..));
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
