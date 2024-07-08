pub use zi_text::Text;
use zi_text::TextSlice;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    Bytes(usize),
}

pub fn indent(config: Config, text: &(impl Text + ?Sized), line_idx: usize) -> Indent {
    if line_idx == 0 {
        return Indent::Bytes(0);
    }

    let Some(line) = text.line(line_idx - 1) else {
        return Indent::Bytes(0);
    };

    let mut indent = line.indent();
    if line.chars().next_back() == Some('{') {
        indent += config.tab_width as usize;
    }

    Indent::Bytes(indent)
}

pub struct Config {
    pub tab_width: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self { tab_width: 4 }
    }
}
