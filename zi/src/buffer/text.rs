use super::*;
use crate::text::{AnyTextSlice, Text};

pub struct TextBuffer<X> {
    id: BufferId,
    flags: BufferFlags,
    path: PathBuf,
    /// The resource url of this buffer
    url: Url,
    /// The url of the file (effectively a cached copy of `Url::from_file_path(&self.path)`)
    file_url: Option<Url>,
    text: X,
    language_id: FileType,
    syntax: Option<Syntax>,
    // FIXME highlight map doesn't belong here
    highlight_map: HighlightMap,
    version: u32,
    tab_width: u8,
}

impl<X: Text + 'static> Buffer for TextBuffer<X> {
    #[inline]
    fn id(&self) -> BufferId {
        self.id
    }

    #[inline]
    fn flags(&self) -> BufferFlags {
        self.flags
    }

    #[inline]
    fn path(&self) -> &Path {
        &self.path
    }

    #[inline]
    fn url(&self) -> &Url {
        &self.url
    }

    #[inline]
    fn file_url(&self) -> Option<&Url> {
        self.file_url.as_ref()
    }

    #[inline]
    fn file_type(&self) -> &FileType {
        &self.language_id
    }

    #[inline]
    fn tab_width(&self) -> u8 {
        self.tab_width
    }

    #[inline]
    fn text(&self) -> &dyn AnyText {
        &self.text
    }

    fn edit(&mut self, delta: &Delta<'_>) -> Result<(), ropey::Error> {
        match self.text.as_text_mut() {
            Some(text) => {
                if let Some(syntax) = self.syntax.as_mut() {
                    syntax.edit(text, delta)?
                } else {
                    text.edit(delta)?
                }

                self.version.checked_add(1).unwrap();
            }
            // FIXME need to check flags and prevent this
            None => panic!("trying to modify a readonly buffer: {}", std::any::type_name::<X>()),
        }

        Ok(())
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
                            self.text.byte_slice(..).dyn_get_line(idx).unwrap().len_bytes()
                        };
                        (Range::new(Point::new(idx, start), Point::new(idx, end)), id)
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

impl<X: AnyText> TextBuffer<X> {
    #[inline]
    pub fn new(
        id: BufferId,
        flags: BufferFlags,
        language_id: FileType,
        path: impl AsRef<Path>,
        mut text: X,
        theme: &Theme,
    ) -> Self {
        let path = path.as_ref();
        let path = std::fs::canonicalize(path).ok().unwrap_or_else(|| path.to_path_buf());
        let file_url = Url::from_file_path(&path).ok();

        let url = file_url.as_ref().map_or_else(
            // maybe there's another reason a buffer wouldn't have a url?
            || Url::parse("buffer://zi/scratch").unwrap(),
            |_url| Url::parse(&format!("buffer://{}", path.display())).unwrap(),
        );

        // ensure the buffer ends with a newline
        if let Some(text) = text.as_text_mut() {
            let idx = text.len_bytes();
            if (text as &dyn AnyText).chars().next_back() != Some('\n') {
                text.edit(&Delta::insert_at(idx, "\n")).unwrap();
            }
        } else if !flags.contains(BufferFlags::READONLY) {
            panic!("must set readonly buffer flag for readonly text implementations")
        }

        let mut syntax = match Syntax::for_language(&language_id) {
            Ok(syntax) => syntax,
            Err(err) => {
                // TODO show the error somewhere
                tracing::error!("failed to load syntax for {}: {}", language_id, err);
                None
            }
        };

        if let Some(syntax) = &mut syntax {
            syntax.set(&text);
        }

        Self {
            id,
            flags,
            path,
            url,
            file_url,
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
