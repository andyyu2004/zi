use std::mem;

use zi_text::{AnyTextSlice, Text, TextMut, TextSlice};

use super::*;
use crate::syntax::{HighlightMap, HighlightName};
use crate::undo::UndoTree;

pub struct TextBuffer<X> {
    id: BufferId,
    flags: BufferFlags,
    /// The resource url of this buffer
    url: Url,
    /// The url of the file (effectively a cached copy of `Url::from_file_path(&self.path)`)
    file_url: Option<Url>,
    text: X,
    language_id: FileType,
    syntax: Option<Syntax>,
    highlight_map: HighlightMap,
    version: u32,
    config: Settings,
    undo_tree: UndoTree<UndoEntry>,
    /// Changes to the buffer that have not been saved to the undo tree
    changes: Vec<Change>,
    saved_cursor: Option<Point>,
}

impl<X: Text + Clone + 'static> BufferHistory for TextBuffer<X> {
    fn undo(&mut self) -> Option<UndoEntry> {
        // Nothing to undo if the buffer is readonly
        let _text = self.text.as_text_mut()?;

        // Snapshot any pending changes before undoing
        self.snapshot(SnapshotFlags::empty());
        self.undo_tree.undo().cloned()
    }

    fn redo(&mut self) -> Option<UndoEntry> {
        // Nothing to redo if the buffer is readonly
        let _text = self.text.as_text_mut()?;
        // Snapshot any pending changes before redoing
        self.snapshot(SnapshotFlags::empty());
        self.undo_tree.redo().cloned()
    }

    fn clear(&mut self) {
        self.changes.clear();
        self.undo_tree.clear();
    }

    #[tracing::instrument(skip(self))]
    fn snapshot(&mut self, flags: SnapshotFlags) {
        if !flags.contains(SnapshotFlags::ALLOW_EMPTY) && self.changes.is_empty() {
            return;
        }

        let changes = mem::take(&mut self.changes);
        tracing::debug!(?flags, ?changes, "snapshot buffer");

        self.undo_tree
            .push(UndoEntry { changes: changes.into(), cursor: self.saved_cursor.take() });
    }

    fn snapshot_cursor(&mut self, cursor: Point) {
        self.saved_cursor = Some(cursor);
    }
}

impl<X: Text + Clone + Send + 'static> Buffer for TextBuffer<X> {
    #[inline]
    fn id(&self) -> BufferId {
        self.id
    }

    #[inline]
    fn flags(&self) -> BufferFlags {
        self.flags
    }

    #[inline]
    fn flushed(&mut self, _: Internal) {
        self.flags.remove(BufferFlags::DIRTY);
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
    fn file_type(&self) -> FileType {
        self.language_id
    }

    #[inline]
    fn settings(&self) -> &Settings {
        &self.config
    }

    #[inline]
    fn text(&self) -> &(dyn AnyText + 'static) {
        &self.text
    }

    #[inline]
    fn syntax(&self) -> Option<&Syntax> {
        self.syntax.as_ref()
    }

    fn edit_flags(&mut self, _: Internal, deltas: &Deltas<'_>, flags: EditFlags) {
        self.edit(deltas, flags);
    }

    fn version(&self) -> u32 {
        self.version
    }

    fn history_mut(&mut self, _: Internal) -> Option<&mut dyn BufferHistory> {
        Some(self)
    }

    fn syntax_highlights<'a>(
        &'a self,
        _editor: &Editor,
        cursor: &'a mut QueryCursor,
        range: PointRange,
    ) -> Box<dyn Iterator<Item = SyntaxHighlight> + 'a> {
        let Some(syntax) = &self.syntax else {
            return Box::new(std::iter::empty());
        };

        Box::new(syntax.highlights(cursor, &self.text, range).flat_map(move |capture| {
            let range = capture.node.range();
            let id = self.highlight_map.get(capture.index);
            // Split multi-line highlights into single-line highlights
            (range.start_point.row..=range.end_point.row).map(move |idx| {
                let start = if idx == range.start_point.row { range.start_point.column } else { 0 };
                let end = if idx == range.end_point.row {
                    range.end_point.column
                } else {
                    self.text.byte_slice(..).dyn_line(idx).unwrap().len_bytes()
                };

                SyntaxHighlight {
                    range: PointRange::new(Point::new(idx, start), Point::new(idx, end)),
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
        let end = match text.line(cursor.line()) {
            Some(line) => {
                let line_byte_width =
                    line.chars().take(size.width as usize).map(|c| c.len_utf8()).sum::<usize>();
                size.width.max(line_byte_width as u16)
            }
            _ => size.width,
        } as usize;

        // The current_line highlight
        Box::new(std::iter::once(Highlight {
            range: PointRange::new(cursor.with_col(0), cursor.with_col(end)),
            id: editor.highlight_id_by_name(HighlightName::CURSORLINE),
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self, _: Internal) -> &mut dyn Any {
        self
    }
}

impl<X: Text + Clone> TextBuffer<X> {
    #[inline]
    pub fn new(
        id: BufferId,
        flags: BufferFlags,
        ft: FileType,
        path: impl AsRef<Path>,
        mut text: X,
        theme: &Theme,
    ) -> Self {
        let path = path.as_ref();
        let path = std::fs::canonicalize(path).ok().unwrap_or_else(|| path.to_path_buf());
        let file_url = Url::from_file_path(&path).ok();

        let url = file_url.as_ref().map_or_else(
            // maybe there's another reason a buffer wouldn't have a url?
            || Url::parse("buffer://scratch").unwrap(),
            |_url| Url::parse(&format!("buffer://{}", path.display())).unwrap(),
        );

        if text.as_text_mut().is_none() && !flags.contains(BufferFlags::READONLY) {
            panic!("must set readonly buffer flag for readonly text implementations")
        }

        let mut syntax = match Syntax::for_file_type(ft) {
            Ok(syntax) => syntax,
            Err(err) => {
                // TODO show the error somewhere
                tracing::error!("failed to load syntax for {ft}: {err}");
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
            url,
            file_url,
            text,
            syntax,
            language_id: ft,
            highlight_map,
            config: Default::default(),
            changes: Default::default(),
            version: Default::default(),
            undo_tree: Default::default(),
            saved_cursor: Default::default(),
        }
    }

    // Ensure the buffer still with a newline.
    // It's fine to do this without editing the syntax as a trailing
    // newline won't affect it.
    fn ensure_trailing_newline(&mut self) {
        if self.text.chars().next_back() != Some('\n') {
            let len = self.text.len_bytes();
            self.edit(&Deltas::insert_at(len, "\n"), EditFlags::NO_ENSURE_NEWLINE);
        }
    }

    fn edit(&mut self, deltas: &Deltas<'_>, flags: EditFlags) {
        // Ensure the buffer ends with a newline before any insert.
        let should_ensure_newline =
            !flags.contains(EditFlags::NO_ENSURE_NEWLINE) && deltas.has_inserts();

        if should_ensure_newline {
            self.ensure_trailing_newline();
        }

        tracing::trace!(?flags, ?deltas, "edit buffer");

        match self.text.as_text_mut() {
            Some(text) => {
                if !deltas.is_identity() {
                    tracing::debug!("set buffer dirty");
                    self.flags.insert(BufferFlags::DIRTY);
                }

                let (inversion, _prev_tree) = match self.syntax.as_mut() {
                    Some(syntax) => syntax.edit(text, deltas),
                    None => (text.edit(deltas), None),
                };

                if !flags.contains(EditFlags::NO_RECORD) {
                    self.changes.push(Change { deltas: deltas.to_owned(), inversions: inversion });
                }

                self.version.checked_add(1).unwrap();
            }
            // FIXME need to check flags and prevent this
            None => panic!("trying to modify a readonly buffer: {}", std::any::type_name::<X>()),
        }

        // If the buffer is empty then we leave it so.
        if should_ensure_newline && self.text.is_empty() {
            self.ensure_trailing_newline();
        }
    }
}
