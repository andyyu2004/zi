use std::ops::Range;

use zi_text::{AnyText, Text, TextSlice};
use zi_textobject::{TextObject, TextObjectFlags, TextObjectKind};

use crate::{Operator, Point};

pub(super) struct OperatorTarget {
    pub(super) range: Range<usize>,
    pub(super) kind: TextObjectKind,
    /// The start before extending the range to whole lines.
    pub(super) start: Point,
    pub(super) start_char: Option<char>,
    pub(super) end_adjusted: bool,
}

impl OperatorTarget {
    pub(super) fn resolve(
        obj: &impl TextObject,
        text: &dyn AnyText,
        cursor: Point,
        operator: Operator,
    ) -> Option<Self> {
        let mut range = obj.byte_range(text, text.point_to_byte(cursor))?;
        let default_kind = obj.default_kind();
        let mut kind = default_kind;
        let flags = obj.flags();
        let start_char = text.char_at_byte(range.start);
        let start = text.byte_to_point(range.start);
        let end = if flags.contains(TextObjectFlags::INCLUSIVE) && !range.is_empty() {
            let last = text.char_before_byte(range.end).map_or(0, char::len_utf8);
            text.byte_to_point(range.end - last)
        } else {
            text.byte_to_point(range.end)
        };
        let mut line_count = end.line() - start.line() + 1;

        let end_adjusted = flags.contains(TextObjectFlags::EXCLUSIVE)
            && end.col() == 0
            && kind == TextObjectKind::Charwise
            && line_count > 1;

        if end_adjusted {
            // An exclusive endpoint at column zero stops before the preceding line terminator.
            // If the start is in the indent, the motion instead becomes linewise.
            // https://github.com/neovim/neovim/blob/2088521263d3bf9cfd23729adb1a7d152eaab104/src/nvim/ops.c#L6073-L6098
            line_count -= 1;
            if text.inindent(start) {
                kind = TextObjectKind::Linewise;
            } else {
                let line = end.line() - 1;
                let line_above = text.line(line).expect("must be in-bounds");
                let adjusted_end = text.point_to_byte(Point::new(line, line_above.len_bytes()));
                if adjusted_end > range.start {
                    range.end = adjusted_end;
                }
            }
        }

        // vi also promotes a multi-line, non-inclusive delete ending in the indent of a
        // whitespace-only line to a linewise delete.
        // https://github.com/neovim/neovim/blob/efb44e0cad294f51e330d57d7590d38de5cec62c/src/nvim/ops.c#L1468-L1484
        if kind == TextObjectKind::Charwise
            && line_count > 1
            && operator == Operator::Delete
            && !flags.contains(TextObjectFlags::INCLUSIVE)
            && text.inindent(end)
            && text.line(end.line()).unwrap().chars().all(char::is_whitespace)
        {
            kind = TextObjectKind::Linewise;
        }

        if default_kind == TextObjectKind::Charwise && kind == TextObjectKind::Linewise {
            range = text.line_to_byte(start.line())..text.line_to_byte(start.line() + line_count);
        }

        Some(Self { range, kind, start, start_char, end_adjusted })
    }
}
