use super::*;

#[derive(Debug, Clone)]
pub struct MatchIt;

impl TextObject for MatchIt {
    fn byte_range(&self, _text: &dyn AnyText, _byte: usize) -> Option<ops::Range<usize>> {
        todo!()
    }

    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }
}

impl Motion for MatchIt {
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        let Some(a) = text.char_at_point_or_byte(p) else { return p };
        let (target, forward) = match a {
            '(' => (')', true),
            ')' => ('(', false),
            '[' => (']', true),
            ']' => ('(', false),
            '{' => ('}', true),
            '}' => ('{', false),
            '<' => ('>', true),
            '>' => ('<', false),
            _ => return p,
        };

        let mut depth = 0;
        let mut byte = text.point_or_byte_to_byte(p);
        match forward {
            true => {
                for c in text.byte_slice(byte..).chars() {
                    if c == a {
                        depth += 1;
                    } else if c == target {
                        assert!(depth > 0);
                        depth -= 1;
                        if depth == 0 {
                            return byte.into();
                        }
                    }

                    byte += c.len_utf8();
                }
            }
            false => {
                for c in text.byte_slice(..byte).chars().rev() {
                    byte -= c.len_utf8();
                    if c == target {
                        if depth == 0 {
                            return byte.into();
                        }
                        assert!(depth > 0);
                        depth -= 1;
                    } else if c == a {
                        depth += 1;
                    }
                }
            }
        }

        // Otherwise not found, return the original point
        p
    }
}
