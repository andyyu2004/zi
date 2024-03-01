#![feature(try_blocks, type_alias_impl_trait, trait_upcasting)]

mod boring_impls;
mod buffer;
mod editor;
pub mod event;
pub mod input;
mod keymap;
mod language;
mod layout;
mod lsp;
mod motion;
mod plugin;
mod position;
mod syntax;
mod view;

pub use tree_sitter;

pub use self::buffer::{Buffer, BufferId};
pub use self::editor::{Callbacks, Editor};
pub use self::language::{LanguageId, LanguageServerId};
pub use self::position::{Col, Direction, Line, Location, Position, Range, Size};
pub use self::syntax::{Color, Style};
pub use self::view::{View, ViewId};

// Consider a proper error type
pub type Error = anyhow::Error;
pub type Result<T> = std::result::Result<T, Error>;

wasmtime::component::bindgen!({
    // async: true,
    // tracing: true,
    // ownership: Borrowing {
    //     duplicate_if_necessary: true
    // },
});

// FIXME make this configurable
pub const TAB_WIDTH: u32 = 4;
