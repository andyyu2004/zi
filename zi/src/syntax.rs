mod highlight;

use std::sync::OnceLock;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use tree_sitter::{Node, Parser, Query, QueryCapture, QueryCaptures, QueryCursor, Tree};

pub use self::highlight::{Color, Style};
pub(crate) use self::highlight::{HighlightId, HighlightMap, Theme};
use crate::buffer::Text;
use crate::FileType;

pub struct Syntax {
    highlights_query: &'static Query,
    tree: Option<Tree>,
    parser: Parser,
}

/// A cache of tree-sitter queries for each language.
/// Creating a query is very expensive, so we cache them here forever.
/// Not concerned about memory usage because the queries are not large, and there are not many languages.
static QUERY_CACHE: OnceLock<RwLock<FxHashMap<tree_sitter::Language, &'static Query>>> =
    OnceLock::new();

impl Syntax {
    pub fn for_language(id: &FileType) -> Option<Self> {
        let (language, highlights) = match id {
            id if *id == FileType::RUST => {
                (tree_sitter_rust::language(), tree_sitter_rust::HIGHLIGHT_QUERY)
            }
            id if *id == FileType::GO => {
                (tree_sitter_go::language(), tree_sitter_go::HIGHLIGHT_QUERY)
            }
            id if *id == FileType::TOML => {
                (tree_sitter_toml::language(), tree_sitter_toml::HIGHLIGHT_QUERY)
            }
            id if *id == FileType::JSON => {
                (tree_sitter_json::language(), tree_sitter_json::HIGHLIGHT_QUERY)
            }
            _ => return None,
        };

        let cache = QUERY_CACHE.get_or_init(Default::default);
        let read_guard = cache.read();
        let highlights_query = match read_guard.get(&language) {
            Some(&query) => query,
            None => {
                drop(read_guard);
                let query =
                    Query::new(language, highlights).expect("failed to create tree-sitter query");
                let query = &*Box::leak(Box::new(query));
                cache.write().insert(language, query);
                query
            }
        };

        let mut parser = Parser::new();
        parser.set_language(language).expect("failed to set tree-sitter parser language");
        parser.set_timeout_micros(5000);
        Some(Self { highlights_query, parser, tree: None })
    }

    pub fn apply(&mut self, source: &dyn Text) {
        self.tree = self.parser.parse_with(
            &mut |byte, _point| source.chunk_at_byte(byte),
            None,
            // TODO incremental partial parsing
            // i.e. keep tree in sync and pass it in, also can only parse the visible range of the document
            // self.tree.as_ref(),
        );
    }

    pub fn highlights<'a, 'tree: 'a>(
        &'tree self,
        cursor: &'a mut QueryCursor,
        source: &'a dyn Text,
    ) -> Highlights<'a, 'tree> {
        match &self.tree {
            Some(tree) => {
                let captures =
                    cursor.captures(self.highlights_query, tree.root_node(), TextProvider(source));
                Highlights::Captures(captures)
            }
            None => Highlights::Empty,
        }
    }

    pub fn highlights_query(&self) -> &'static Query {
        self.highlights_query
    }
}

/// A wrapper type that allows us to construct an empty iterator if we have no highlights to provide
#[derive(Default)]
pub enum Highlights<'a, 'tree> {
    Captures(QueryCaptures<'a, 'tree, TextProvider<'a>>),
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

pub struct TextProvider<'a>(&'a dyn Text);

impl<'a> tree_sitter::TextProvider<'a> for TextProvider<'a> {
    type I = std::iter::Map<Box<dyn Iterator<Item = &'a str> + 'a>, fn(&'a str) -> &'a [u8]>;

    fn text(&mut self, node: Node<'_>) -> Self::I {
        self.0.byte_slice(node.start_byte()..node.end_byte()).map(str::as_bytes)
    }
}
