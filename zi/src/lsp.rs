mod proto;

use std::ops::ControlFlow;

use zi_lsp::{ResponseError, Result};

pub(crate) struct LanguageClient;

impl zi_lsp::LanguageClient for LanguageClient {
    type Error = ResponseError;

    type NotifyResult = ControlFlow<Result<()>>;
}
