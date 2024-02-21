#![feature(try_blocks)]

mod boring_impls;
mod buffer;
mod editor;
pub mod event;
mod keymap;
mod plugin;
mod position;
mod view;

pub use self::buffer::{Buffer, BufferId};
pub use self::editor::Editor;
pub use self::position::{Col, Direction, Line, Position};
pub use self::view::{View, ViewId};
pub use self::zi::zi::editor::Mode;

wasmtime::component::bindgen!({
    async: true,
    // tracing: true,
    ownership: Borrowing {
        duplicate_if_necessary: true
    },
});
