use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindKind {
    Find,
    FindBack,
    Till,
    TillBack,
}

#[derive(Debug, Clone, Copy)]
pub struct FindChar {
    pub kind: FindKind,
    pub target: char,
}

impl FindChar {
    fn target_byte(self, text: &dyn AnyText, byte: usize, n: usize) -> Option<usize> {
        let mut seen = 0;
        match self.kind {
            FindKind::Find | FindKind::Till => {
                let mut graphemes = graphemes_from(text, byte);
                let cursor = graphemes.next().filter(|g| g.first != '\n')?;
                let mut prev = byte;
                let mut start = byte + cursor.len;
                for Grapheme { first, len } in graphemes {
                    if first == '\n' {
                        return None;
                    }

                    if first == self.target {
                        seen += 1;
                        if seen == n {
                            return Some(match self.kind {
                                FindKind::Find => start,
                                FindKind::Till => prev,
                                FindKind::FindBack | FindKind::TillBack => unreachable!(),
                            });
                        }
                    }

                    prev = start;
                    start += len;
                }
                None
            }
            FindKind::FindBack | FindKind::TillBack => {
                let mut end = byte;
                for Grapheme { first, len } in graphemes_before(text, byte) {
                    if first == '\n' {
                        return None;
                    }

                    let start = end - len;
                    if first == self.target {
                        seen += 1;
                        if seen == n {
                            return Some(match self.kind {
                                FindKind::FindBack => start,
                                FindKind::TillBack => end,
                                FindKind::Find | FindKind::Till => unreachable!(),
                            });
                        }
                    }

                    end = start;
                }
                None
            }
        }
    }
}

impl TextObject for FindChar {
    fn byte_range(&self, text: &dyn AnyText, byte: usize) -> Option<ops::Range<usize>> {
        self.byte_range_n(text, byte, 1)
    }

    fn byte_range_n(&self, text: &dyn AnyText, byte: usize, n: usize) -> Option<ops::Range<usize>> {
        let target = self.target_byte(text, byte, n)?;
        match self.kind {
            FindKind::Find | FindKind::Till => {
                let len = graphemes_from(text, target).next().map_or(0, |g| g.len);
                Some(byte..target + len)
            }
            FindKind::FindBack | FindKind::TillBack => Some(target..byte),
        }
    }

    fn default_kind(&self) -> TextObjectKind {
        TextObjectKind::Charwise
    }

    fn flags(&self) -> TextObjectFlags {
        match self.kind {
            FindKind::Find | FindKind::Till => TextObjectFlags::empty(),
            FindKind::FindBack | FindKind::TillBack => TextObjectFlags::EXCLUSIVE,
        }
    }
}

impl Motion for FindChar {
    fn motion(&self, text: &dyn AnyText, p: PointOrByte) -> PointOrByte {
        self.motion_n(text, p, 1)
    }

    fn motion_n(&self, text: &dyn AnyText, p: PointOrByte, n: usize) -> PointOrByte {
        let byte = text.point_or_byte_to_byte(p);
        self.target_byte(text, byte, n).map_or(p, PointOrByte::from)
    }
}
