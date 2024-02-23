use ropey::RopeSlice;

use crate::Position;

pub trait Motion {
    fn motion(&mut self, slice: RopeSlice<'_>, pos: Position) -> Position;
}

pub struct NextWord;

impl Motion for NextWord {
    fn motion(&mut self, slice: RopeSlice<'_>, pos: Position) -> Position {
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
