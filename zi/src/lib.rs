#![feature(try_blocks, type_alias_impl_trait, coroutines, iter_from_coroutine)]

pub use zi_input as input;

pub mod buffer;
pub mod command;
mod completion;
mod config;
pub mod dirs;
mod editor;
pub mod event;
mod jump;
mod keymap;
mod language;
mod language_service;
mod layout;
mod location;
mod namespace;
mod operator;
pub mod plugin;
mod private;
mod syntax;
mod undo;
pub mod view;

pub use location::Location;
pub use tokio::sync::Notify;
pub use tree_sitter;
pub use tui::{Constraint, LineNumberStyle};
pub use url::Url;
pub use zi_core::{
    BufferId, Col, CompletionItem, Direction, Line, Mode, NamespaceId, Offset, Operator, Point,
    PointRange, Size, ViewGroupId, ViewId,
};
pub use zi_text::{
    AnyText, AnyTextMut, AnyTextSlice, Delta, Deltas, Rope, RopeBuilder, Text, TextBase, TextMut,
    TextSlice, deltas,
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
pub use self::language_service::{LanguageClient, LanguageService, LanguageServiceConfig, lstypes};
pub use self::namespace::Namespace;
#[doc(hidden)]
pub use self::syntax::HighlightName;
pub use self::syntax::{Syntax, Theme};
pub use self::view::{VerticalAlignment, View};

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
//
