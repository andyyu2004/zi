use futures_core::future::BoxFuture;
use zi_lsp::lsp_types;

use crate::Result;
// TODO using lsp_types for now, but should define our own interface to drop the dependency;

/// An abstraction of language server requests. Notifications are handled via the event system.
pub trait LanguageService {
    fn initialize(&self, params: lsp_types::InitializeParams) -> ResponseFuture<()>;
}

pub type ResponseFuture<T> = BoxFuture<'static, Result<T>>;
