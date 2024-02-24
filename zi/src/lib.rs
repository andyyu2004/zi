#![feature(try_blocks, type_alias_impl_trait)]

mod boring_impls;
mod buffer;
mod editor;
pub mod event;
mod keymap;
mod language;
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
pub use self::position::{Col, Direction, Line, Position};
pub use self::syntax::{Color, Style};
pub use self::view::{View, ViewId};

wasmtime::component::bindgen!({
    async: true,
    // tracing: true,
    // ownership: Borrowing {
    //     duplicate_if_necessary: true
    // },
});
