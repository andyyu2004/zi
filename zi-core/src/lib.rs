pub mod completion;
pub mod dirs;
mod position;
pub mod style;

pub use self::completion::CompletionItem;
pub use self::position::*;

rel::key! {
    pub struct ViewId;
    pub struct ViewGroupId;
    pub struct BufferId;
    pub struct NamespaceId;
}
