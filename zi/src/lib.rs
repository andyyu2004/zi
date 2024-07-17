#![feature(
    try_blocks,
    type_alias_impl_trait,
    trait_upcasting,
    is_sorted,
    coroutines,
    iter_from_coroutine
)]

pub mod buffer;
mod command;
mod completion;
mod config;
pub mod dirs;
mod editor;
pub mod event;
pub mod input;
mod jump;
mod keymap;
mod language;
mod language_service;
mod layout;
mod location;
#[doc(hidden)]
pub mod lsp;
mod namespace;
mod operator;
mod plugin;
mod private;
mod syntax;
mod undo;
pub mod view;

pub use location::Location;
pub use lsp_types::Url;
pub use tokio::sync::Notify;
pub use tree_sitter;
pub use tui::{Constraint, LineNumberStyle};
pub use zi_core::{
    BufferId, Col, CompletionItem, Diagnostic, Direction, Line, Mode, NamespaceId, Offset,
    Operator, Point, PointRange, PositionEncoding, Severity, Size, ViewGroupId, ViewId,
};
pub use zi_text::{
    deltas, AnyText, AnyTextMut, AnyTextSlice, Delta, Deltas, Rope, RopeBuilder, Text, TextBase,
    TextMut, TextSlice,
};
pub use zi_textobject::motion;

pub use self::buffer::{BufferFlags, Mark, MarkBuilder, MarkId, PickerBuffer, TextBuffer};
pub use self::command::{Command, Commands};
pub use self::completion::CompletionProvider;
pub use self::config::Setting;
pub use self::editor::{
    Active, Backend, Client, DummyBackend, EditError, Editor, Match, OpenFlags, Resource,
    SaveFlags, Tasks,
};
pub(crate) use self::jump::JumpList;
pub use self::language::{FileType, LanguageConfig, LanguageServiceId};
pub use self::language_service::{lstypes, LanguageClient, LanguageService, LanguageServiceConfig};
pub use self::namespace::Namespace;
pub use self::syntax::Syntax;
pub use self::view::{VerticalAlignment, View};
// pub use self::wit::zi::api::editor::{Mode, Operator};

pub type Error = anyhow::Error;
pub type Result<T, E = Error> = std::result::Result<T, E>;

// mod wit {
//     wasmtime::component::bindgen!({
//         async: true,
//         tracing: true,
//         ownership: Borrowing {
//             duplicate_if_necessary: true
//         },
//         with: {}
//     });
// }
