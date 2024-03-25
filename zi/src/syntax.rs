mod highlight;

use std::cell::RefCell;
use std::ops::Bound;
use std::sync::OnceLock;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use tree_sitter::{Node, Parser, Query, QueryCapture, QueryCaptures, QueryCursor, Tree};

pub use self::highlight::{Color, Style};
pub(crate) use self::highlight::{HighlightId, HighlightMap, Theme};
use crate::text::{AnyText, AnyTextMut, AnyTextSlice, Delta, Text, TextSlice};
use crate::{dirs, FileType};

pub struct Syntax {
    language: tree_sitter::Language,
    highlights_query: &'static Query,
    tree: Option<Tree>,
}

/// The wasm engine to use for tree-sitter.
/// We're not using the same engine as the plugins because issues with using async stores with tree-sitter.
static ENGINE: OnceLock<wasmtime::Engine> = OnceLock::new();

thread_local! {
    static PARSER: RefCell<Parser> = {
        let mut parser = Parser::new();
        parser.set_wasm_store(tree_sitter::WasmStore::new(ENGINE.get_or_init(Default::default).clone()).unwrap()).unwrap();
        // parser.set_timeout_micros(10000);

        // TODO we should use this feature as otherwise it times out quite easily
        parser.set_included_ranges(&[]).expect("passed invalid ranges");
        RefCell::new(parser)
    };
}

/// A cache of tree-sitter queries for each language.
/// Creating a query and compiling a language is very expensive, so we cache them here forever.
/// Not concerned about memory usage because these are not large, and there are not many languages.
static QUERY_CACHE: OnceLock<RwLock<FxHashMap<FileType, (tree_sitter::Language, &'static Query)>>> =
    OnceLock::new();

impl Syntax {
    #[tracing::instrument]
    pub fn for_language(ft: &FileType) -> anyhow::Result<Option<Self>> {
        let cache = QUERY_CACHE.get_or_init(Default::default);
        let read_guard = cache.read();
        let (language, highlights_query) = match read_guard.get(ft) {
            Some(cached) => cached.clone(),
            None => {
                drop(read_guard);

                let grammar_dir = dirs::grammar().join(ft);
                let wasm_path = grammar_dir.join("language.wasm");
                let highlights_path = grammar_dir.join("highlights.scm");

                if !wasm_path.exists() || !highlights_path.exists() {
                    tracing::info!(?ft, "no wasm or highlights file found for language");
                    return Ok(None);
                }

                let bytes = std::fs::read(wasm_path)?;
                let language = PARSER.with(|parser| {
                    let mut parser = parser.borrow_mut();
                    let mut store = parser
                        .take_wasm_store()
                        .expect("set during initialization and we always re-set it after");
                    let now = std::time::Instant::now();
                    let language = store.load_language(ft.as_str(), &bytes);
                    tracing::info!(%ft, dur = ?now.elapsed(), "loaded wasm language");
                    parser.set_wasm_store(store).expect("this succeeded during initialization");
                    language
                })?;

                let highlights_text = std::fs::read_to_string(highlights_path)?;
                let highlights_query =
                    &*Box::leak(Box::new(Query::new(&language, &highlights_text)?));
                cache.write().insert(ft.clone(), (language.clone(), highlights_query));
                (language, highlights_query)
            }
        };

        Ok(Some(Self { language, highlights_query, tree: None }))
    }

    /// Set the text of the syntax tree.
    /// Prefer using `edit` if you have a delta.
    pub fn set(&mut self, text: &dyn AnyText) {
        self.tree = PARSER.with(|parser| {
            let mut parser = parser.borrow_mut();
            parser.set_language(&self.language).unwrap();
            parser.parse_with(
                &mut |byte, _point| text.byte_slice(byte..).chunks().next().unwrap_or(""),
                None,
            )
        });
    }

    pub fn edit(&mut self, text: &mut dyn AnyTextMut, delta: &Delta<'_>) {
        match &mut self.tree {
            Some(tree) => tree.edit(&delta_to_ts_edit(text, delta)),
            _ => text.dyn_edit(delta),
        }

        PARSER.with(|parser| {
            let mut parser = parser.borrow_mut();
            parser.set_language(&self.language).unwrap();
            if let Some(tree) = parser.parse_with(
                &mut |byte, _point| text.byte_slice(byte..).chunks().next().unwrap(),
                self.tree.as_ref(),
            ) {
                self.tree = Some(tree);
            }
        });
    }

    pub fn highlights<'a, 'tree: 'a>(
        &'tree self,
        cursor: &'a mut QueryCursor,
        source: &'a dyn AnyText,
    ) -> Highlights<'a, 'tree> {
        match &self.tree {
            Some(tree) => {
                let captures = cursor.captures(
                    self.highlights_query,
                    tree.root_node(),
                    TextProvider(source.dyn_byte_slice((Bound::Unbounded, Bound::Unbounded))),
                );
                Highlights::Captures(captures)
            }
            None => Highlights::Empty,
        }
    }

    pub fn highlights_query(&self) -> &'static Query {
        self.highlights_query
    }
}

// tree-sitter point column is byte-indexed, but very poorly documented
fn delta_to_ts_edit(text: &mut dyn AnyTextMut, delta: &Delta<'_>) -> tree_sitter::InputEdit {
    let byte_range = text.delta_to_byte_range(delta);
    let point_range = text.delta_to_point_range(delta);

    let start_byte = byte_range.start;
    let old_end_byte = byte_range.end;
    let new_end_byte = start_byte + delta.text().len();

    text.dyn_edit(delta);

    let new_end_position = text.byte_to_point(new_end_byte).into();

    tree_sitter::InputEdit {
        start_byte,
        old_end_byte,
        new_end_byte,
        start_position: point_range.start().into(),
        old_end_position: point_range.end().into(),
        new_end_position,
    }
}

/// A wrapper type that allows us to construct an empty iterator if we have no highlights to provide
#[derive(Default)]
pub enum Highlights<'a, 'tree> {
    Captures(QueryCaptures<'a, 'tree, TextProvider<'a>, &'a [u8]>),
    #[default]
    Empty,
}

impl<'a, 'tree: 'a> Iterator for Highlights<'a, 'tree> {
    type Item = QueryCapture<'tree>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Highlights::Captures(captures) => captures.next().map(|(m, idx)| m.captures[idx]),
            Highlights::Empty => None,
        }
    }
}

pub struct TextProvider<'a>(Box<dyn AnyTextSlice<'a> + 'a>);

impl<'a> tree_sitter::TextProvider<&'a [u8]> for TextProvider<'a> {
    type I = std::iter::Map<Box<dyn Iterator<Item = &'a str> + 'a>, fn(&'a str) -> &'a [u8]>;

    fn text(&mut self, node: Node<'_>) -> Self::I {
        let slice = self.0.byte_slice(node.start_byte()..node.end_byte());
        slice.dyn_chunks().map(str::as_bytes)
    }
}

#[cfg(test)]
mod tests;
