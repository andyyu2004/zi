//! Boring impls converting zi to lsp types

use zi_lsp::lsp_types;

use crate::Position;

impl From<Position> for lsp_types::Position {
    fn from(pos: Position) -> Self {
        lsp_types::Position { line: pos.line().raw(), character: pos.col().raw() }
    }
}
