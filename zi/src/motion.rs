use ropey::RopeSlice;

use crate::Position;

pub trait Motion {
    fn motion(self, slice: RopeSlice<'_>, pos: Position) -> Position;
}

pub struct PrevToken;

impl Motion for PrevToken {
    fn motion(self, slice: RopeSlice<'_>, pos: Position) -> Position {
        let start_char = slice.line_to_char(pos.line().idx()) + pos.col().idx();
        let mut chars = slice.chars_at(start_char).reversed();

        let prev = chars.next().unwrap_or('x');
        let mut i = 0;
        for c in chars {
            if c.is_whitespace() && !prev.is_whitespace() {
                break;
            }
            i += 1;
        }

        let char = start_char - i;
        let line = slice.char_to_line(char);
        let col = char - slice.line_to_char(line);
        Position::new(line, col)
    }
}

pub struct NextWord;

impl Motion for NextWord {
    fn motion(self, slice: RopeSlice<'_>, pos: Position) -> Position {
        let start_char = slice.line_to_char(pos.line().idx()) + pos.col().idx();
        let chars = slice.chars_at(start_char);

        let is_sep = |c: char| c.is_whitespace() || !c.is_alphanumeric() || c.is_uppercase();

        let mut i = 0;
        let mut found_sep = false;
        for c in chars {
            if found_sep && !c.is_whitespace() {
                break;
            }

            if is_sep(c) {
                found_sep = true;
            }

            i += 1;
        }

        let char = start_char + i;
        let line = slice.char_to_line(char);
        let col = char - slice.line_to_char(line);
        Position::new(line, col)
    }
}

/// Whitespace delimited word
pub struct NextToken;

impl Motion for NextToken {
    fn motion(self, slice: RopeSlice<'_>, pos: Position) -> Position {
        let start_char = slice.line_to_char(pos.line().idx()) + pos.col().idx();
        let chars = slice.chars_at(start_char);

        let mut i = 0;
        let mut found_whitespace = false;
        for c in chars {
            if found_whitespace && !c.is_whitespace() {
                break;
            }

            if c.is_whitespace() {
                found_whitespace = true;
            }

            i += 1;
        }

        let char = start_char + i;
        let line = slice.char_to_line(char);
        let col = char - slice.line_to_char(line);
        Position::new(line, col)
    }
}
