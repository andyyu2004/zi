use zi_text::Text;

pub struct Indent {
    pub bytes: usize,
}

pub fn indent(text: &(impl Text + ?Sized), line_idx: usize) -> Indent {
    if line_idx == 0 {
        return Indent { bytes: 0 };
    }

    Indent { bytes: text.indent(line_idx - 1) }
}
