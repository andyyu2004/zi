use super::*;
use crate::syntax::{HighlightMap, HighlightName};
use crate::text::{AnyTextSlice, Text, TextMut, TextSlice};
use crate::undo::UndoTree;

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
    undo_tree: UndoTree<UndoEntry>,
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

    #[inline]
    fn syntax(&self) -> Option<&Syntax> {
        self.syntax.as_ref()
    }

    fn edit(&mut self, cursor: Point, delta: &Delta<'_>) {
        self.edit(cursor, delta, EditFlags::PUSH_UNDO);
    }

    fn version(&self) -> u32 {
        self.version
    }

    fn undo(&mut self) -> Option<UndoEntry> {
        // Nothing to undo if the buffer is readonly
        let _text = self.text.as_text_mut()?;

        tracing::debug!("{:#?}", self.undo_tree);
        let entry = self.undo_tree.undo().cloned()?;
        self.edit(entry.cursor, &entry.inverse_delta, EditFlags::NO_APPEND_NEWLINE);
        Some(entry)
    }

    fn redo(&mut self) -> Option<UndoEntry> {
        // Nothing to redo if the buffer is readonly
        let _text = self.text.as_text_mut()?;

        let entry = self.undo_tree.redo().cloned()?;
        self.edit(entry.cursor, &entry.delta, EditFlags::NO_APPEND_NEWLINE);
        Some(entry)
    }

    fn clear_undo(&mut self) {
        self.undo_tree.clear();
    }

    fn syntax_highlights<'a>(
        &'a self,
        _editor: &Editor,
        cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = SyntaxHighlight> + 'a> {
        let Some(syntax) = &self.syntax else {
            return Box::new(std::iter::empty());
        };

        Box::new(syntax.highlights(cursor, &self.text).flat_map(move |capture| {
            let range = capture.node.range();
            let id = self.highlight_map.get(capture.index);
            // Split multi-line highlights into single-line highlights
            (range.start_point.row..=range.end_point.row).map(move |idx| {
                let start = if idx == range.start_point.row { range.start_point.column } else { 0 };
                let end = if idx == range.end_point.row {
                    range.end_point.column
                } else {
                    self.text.byte_slice(..).dyn_get_line(idx).unwrap().len_bytes()
                };

                SyntaxHighlight {
                    range: Range::new(Point::new(idx, start), Point::new(idx, end)),
                    capture_idx: capture.index,
                    id,
                }
            })
        }))
    }

    fn overlay_highlights(
        &self,
        editor: &Editor,
        view: &View,
        size: Size,
    ) -> Box<dyn Iterator<Item = Highlight> + '_> {
        assert_eq!(view.buffer(), self.id);
        let cursor = view.cursor();
        let text = editor.buffer(view).text();

        // We have to be careful that we don't set the end column to the middle of a char
        let end = match text.get_line(cursor.line().idx()) {
            Some(line) => {
                let line_byte_width =
                    line.chars().take(size.width as usize).map(|c| c.len_utf8()).sum::<usize>();
                size.width.max(line_byte_width as u16) as u32
            }
            _ => size.width as u32,
        };

        // The current_line highlight
        Box::new(std::iter::once(Highlight {
            range: Range::new(cursor.with_col(0), cursor.with_col(end)),
            id: editor.highlight_id_by_name(HighlightName::CURSORLINE),
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl<X: Text> TextBuffer<X> {
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

        if text.as_text_mut().is_none() && !flags.contains(BufferFlags::READONLY) {
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

        let highlight_map = HighlightMap::new(
            syntax.as_ref().map_or(&[][..], |syntax| syntax.capture_names()),
            theme,
        );

        Self {
            id,
            flags,
            path,
            url,
            file_url,
            text,
            language_id,
            syntax,
            tab_width: 4,
            highlight_map,
            version: Default::default(),
            undo_tree: Default::default(),
        }
    }

    fn edit(&mut self, cursor: Point, delta: &Delta<'_>, flags: EditFlags) {
        if delta.is_identity() {
            return;
        }

        match self.text.as_text_mut() {
            Some(text) => {
                if !flags.contains(EditFlags::NO_APPEND_NEWLINE)
                    && !delta.text().is_empty()
                    && text.chars().next_back() != Some('\n')
                {
                    // Ensure the buffer ends with a newline before any insert.
                    // It's fine to do this without editing the syntax as a trailing
                    // newline will affect it.
                    text.edit(&Delta::insert_at(text.len_bytes(), "\n"));
                }

                let (inverse_delta, _prev_tree) = if let Some(syntax) = self.syntax.as_mut() {
                    syntax.edit(text, delta)
                } else {
                    (text.edit(delta), None)
                };

                if flags.contains(EditFlags::PUSH_UNDO) {
                    self.undo_tree.push(UndoEntry {
                        cursor,
                        delta: delta.to_owned(),
                        inverse_delta,
                    });

                    tracing::debug!("{:#?}", self.undo_tree);
                }

                self.version.checked_add(1).unwrap();
            }
            // FIXME need to check flags and prevent this
            None => panic!("trying to modify a readonly buffer: {}", std::any::type_name::<X>()),
        }
    }
}
