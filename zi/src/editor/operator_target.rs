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
    /// Whether an exclusive endpoint at column zero excluded its line.
    pub(super) end_adjusted: bool,
}

impl OperatorTarget {
    pub(super) fn resolve(
        obj: &impl TextObject,
        text: &dyn AnyText,
        cursor: Point,
        operator: Operator,
    ) -> Option<Self> {
        let range = obj.byte_range(text, text.point_to_byte(cursor))?;
        let default_kind = obj.default_kind();
        let flags = obj.flags();
        let end = object_end_point(text, &range, flags);
        let mut target = Self {
            start: text.byte_to_point(range.start),
            start_char: text.char_at_byte(range.start),
            range,
            kind: default_kind,
            end_adjusted: false,
        };

        // Apply the exclusive-motion rule before deciding whether a delete spans multiple lines.
        let last_line = target.adjust_exclusive_end(text, end, flags);
        target.promote_blank_line_delete(text, end, last_line, flags, operator);
        if default_kind == TextObjectKind::Charwise && target.kind == TextObjectKind::Linewise {
            target.expand_to_whole_lines(text, last_line);
        }
        Some(target)
    }

    /// Apply [Vim's column-zero rule] and return the last line to count for the operator.
    ///
    /// [Vim's column-zero rule]: https://github.com/neovim/neovim/blob/2088521263d3bf9cfd23729adb1a7d152eaab104/src/nvim/ops.c#L6073-L6098
    fn adjust_exclusive_end(
        &mut self,
        text: &dyn AnyText,
        end: Point,
        flags: TextObjectFlags,
    ) -> usize {
        if self.kind != TextObjectKind::Charwise
            || !flags.contains(TextObjectFlags::EXCLUSIVE)
            || end.byte_col() != 0
            || self.start.line() >= end.line()
        {
            return end.line();
        }

        self.end_adjusted = true;
        let last_line = end.line() - 1;
        if text.inindent(self.start) {
            self.kind = TextObjectKind::Linewise;
        } else {
            // Stop before the previous line's terminator, unless that would empty the range.
            let line = text.line(last_line).expect("must be in-bounds");
            let trimmed_end = text.point_to_byte(Point::new(last_line, line.len_bytes()));
            if trimmed_end > self.range.start {
                self.range.end = trimmed_end;
            }
        }
        last_line
    }

    // A non-inclusive, multi-line delete ending in a whitespace-only line's indent is linewise.
    // `end` is the motion's endpoint; `last_line` may exclude that endpoint's line.
    // https://github.com/neovim/neovim/blob/efb44e0cad294f51e330d57d7590d38de5cec62c/src/nvim/ops.c#L1468-L1484
    fn promote_blank_line_delete(
        &mut self,
        text: &dyn AnyText,
        end: Point,
        last_line: usize,
        flags: TextObjectFlags,
        operator: Operator,
    ) {
        if self.kind == TextObjectKind::Charwise
            && self.start.line() < last_line
            && operator == Operator::Delete
            && !flags.contains(TextObjectFlags::INCLUSIVE)
            && text.inindent(end)
            && text.line(end.line()).unwrap().chars().all(char::is_whitespace)
        {
            self.kind = TextObjectKind::Linewise;
        }
    }

    fn expand_to_whole_lines(&mut self, text: &dyn AnyText, last_line: usize) {
        self.range = text.line_to_byte(self.start.line())..text.line_to_byte(last_line + 1);
    }
}

/// Inclusive objects count the last selected character, not the byte just past the selection.
fn object_end_point(text: &dyn AnyText, range: &Range<usize>, flags: TextObjectFlags) -> Point {
    let byte = if flags.contains(TextObjectFlags::INCLUSIVE) && !range.is_empty() {
        range.end - text.char_before_byte(range.end).map_or(0, char::len_utf8)
    } else {
        range.end
    };
    text.byte_to_point(byte)
}
