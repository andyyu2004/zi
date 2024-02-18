pub use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::widgets;
pub use ratatui::Terminal;

pub trait Widget: widgets::Widget {}
