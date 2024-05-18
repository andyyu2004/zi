#![feature(
    try_blocks,
    type_alias_impl_trait,
    trait_upcasting,
    is_sorted,
    coroutines,
    iter_from_coroutine
)]

pub use tui::LineNumberStyle;
mod boring_impls;
pub mod buffer;
mod command;
mod config;
pub mod dirs;
mod editor;
pub mod event;
pub mod input;
mod jump;
mod keymap;
mod language;
mod layout;
mod location;
#[doc(hidden)]
pub mod lsp;
mod operator;
mod plugin;
mod private;
mod symbol;
mod syntax;
mod undo;
pub mod view;

pub use location::Location;
pub use tokio::sync::Notify;
pub use tree_sitter;
pub use tui::Constraint;
pub use zi_core::{Col, Direction, Line, Offset, Point, PointRange, Size};
pub use zi_lsp::lsp_types::Url;
pub use zi_text::{Delta, Deltas};
pub use zi_textobject::motion;

pub use self::buffer::{BufferFlags, BufferId, PickerBuffer, TextBuffer};
pub use self::command::{Command, Commands};
pub use self::editor::{Active, Client, Editor, Match, OpenFlags, SaveFlags, Tasks};
pub(crate) use self::jump::JumpList;
pub use self::language::{FileType, LanguageConfig, LanguageServerConfig, LanguageServerId};
pub use self::view::{VerticalAlignment, View, ViewId};
pub use self::wit::zi::api::editor::{Mode, Operator};

pub type Error = anyhow::Error;
pub type Result<T, E = Error> = std::result::Result<T, E>;

mod wit {
    wasmtime::component::bindgen!({
        async: true,
        tracing: true,
        ownership: Borrowing {
            duplicate_if_necessary: true
        },
        with: {}
    });
}
