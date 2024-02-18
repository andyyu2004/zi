#![feature(try_blocks)]

mod buffer;
mod editor;
pub mod event;
mod keymap;
mod plugin;
mod position;
mod view;

pub use self::buffer::{Buffer, BufferId};
pub use self::editor::{Editor, Mode};
pub use self::position::{Col, Line, Position};
pub use self::view::{View, ViewId};
