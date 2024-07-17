mod highlight;

use tree_sitter::{Query, QueryCapture, QueryCursor, Tree};
use zi_core::PointRange;
use zi_text::{AnyText, AnyTextMut, Deltas};

pub use self::highlight::{HighlightId, HighlightMap, HighlightName, Theme};
use crate::FileType;

pub trait Syntax: Send + Sync + 'static {
    fn file_type(&self) -> FileType;

    fn highlights_query(&self) -> &Query;

    /// Set the text of the syntax tree.
    /// Prefer using `edit` if there is a delta available.
    fn set(&mut self, text: &dyn AnyText);

    /// Apply the given deltas to the text and update the syntax tree.
    /// Returns the inverse deltas and the previous syntax tree.
    fn edit(
        &mut self,
        text: &mut dyn AnyTextMut,
        deltas: &Deltas<'_>,
    ) -> (Deltas<'static>, Option<Tree>);

    fn highlights<'a, 'tree: 'a>(
        &'tree self,
        query_cursor: &'a mut QueryCursor,
        source: &'a dyn AnyText,
        range: PointRange,
    ) -> Box<dyn Iterator<Item = QueryCapture<'tree>> + 'a>;

    fn capture_names(&self) -> &[&str] {
        self.highlights_query().capture_names()
    }

    fn capture_index_to_name(&self, idx: u32) -> &str {
        self.highlights_query().capture_names()[idx as usize]
    }
}

#[cfg(test)]
mod tests;
