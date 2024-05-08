#![feature(
    try_blocks,
    type_alias_impl_trait,
    trait_upcasting,
    is_sorted,
    coroutines,
    iter_from_coroutine
)]

mod boring_impls;
mod buffer;
mod command;
mod dirs;
mod editor;
pub mod event;
pub mod input;
mod jump;
mod keymap;
mod language;
mod layout;
mod location;
mod lsp;
mod operator;
mod plugin;
mod private;
mod syntax;
mod undo;
mod view;

pub use location::Location;
pub use tokio::sync::Notify;
pub use tree_sitter;
pub use tui::Constraint;
pub use zi_core::{Col, Direction, Line, Offset, Point, PointRange, Size};
pub use zi_lsp::lsp_types::Url;
pub use zi_text::Delta;
pub use zi_textobject::motion;

pub use self::buffer::{BufferFlags, BufferId, PickerBuffer, TextBuffer};
pub use self::editor::{Active, Editor, Match, OpenFlags, Tasks};
pub(crate) use self::jump::JumpList;
pub use self::language::{FileType, LanguageServerId};
pub use self::view::{VerticalAlignment, View, ViewId};
pub use self::wit::zi::api::editor::{Mode, Operator};

// Consider a proper error type
pub type Error = anyhow::Error;
pub type Result<T> = std::result::Result<T, Error>;

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
