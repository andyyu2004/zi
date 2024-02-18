use ratatui::widgets::Widget;

pub trait Element: Widget {}

impl<W: Widget> Element for W {}
