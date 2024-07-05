use futures_core::future::BoxFuture;
use zi_core::{CompletionItem, Point};

use crate::{BufferId, Editor};

pub trait CompletionProvider: Send + Sync + 'static {
    fn completions(
        &self,
        editor: &mut Editor,
        params: CompletionParams,
    ) -> BoxFuture<'static, zi_lsp::Result<Vec<CompletionItem>>>;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct CompletionParams {
    pub buf: BufferId,
    pub point: Point,
}
