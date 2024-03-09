use crate::syntax::HighlightId;

#[derive(Debug, Clone)]
pub struct VirtualText {
    highlight: HighlightId,
    text: String,
}

impl VirtualText {
    pub fn new(highlight: HighlightId, text: String) -> Self {
        Self { highlight, text }
    }
}
