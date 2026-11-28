use core::fmt;
use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicUsize};

use futures_util::future::BoxFuture;
use zi_lsp::lsp_types::notification::{self, Notification};
use zi_lsp::lsp_types::request::{self, Request};
use zi_lsp::lsp_types::{self, OneOf, lsp_notification, lsp_request};
use zi_lsp::{ErrorCode, ResponseError, Result};
use zi_test::{TestContext, new};

mod definition;
mod diagnostics;
mod fake_server;
mod format;
mod startup;
mod status;
mod sync;
mod util;

use self::fake_server::*;
use self::util::*;
