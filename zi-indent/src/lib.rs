use zi_text::Text;

pub struct Indent {
    pub amount: usize,
}

pub fn indent(text: &(impl Text + ?Sized), line_idx: usize) -> Indent {
    if line_idx == 0 {
        return Indent { amount: 0 };
    }

    let line = text.line(line_idx - 1).unwrap();
    Indent { amount: 4 }
}
