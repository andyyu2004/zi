use crate::text::{AnyText, Text, TextSlice};
use crate::Point;

pub trait Motion {
    fn motion(&mut self, text: &dyn AnyText, pos: Point) -> Point;

    fn repeated(self, n: usize) -> Repeated<Self>
    where
        Self: Sized,
    {
        Repeated { motion: self, n }
    }
}

pub struct Repeated<M> {
    motion: M,
    n: usize,
}

impl<M: Motion> Motion for Repeated<M> {
    fn motion(&mut self, text: &dyn AnyText, mut pos: Point) -> Point {
        while self.n > 0 {
            pos = self.motion.motion(text, pos);
            self.n -= 1;
        }
        pos
    }
}

pub struct PrevToken;

impl Motion for PrevToken {
    fn motion(&mut self, text: &dyn AnyText, pos: Point) -> Point {
        let mut byte = text.point_to_byte(pos);
        let mut chars = text.byte_slice(..byte).chars();

        let prev = chars.next_back().unwrap_or('x');
        for c in chars.rev() {
            if c.is_whitespace() && !prev.is_whitespace() {
                break;
            }
            byte -= c.len_utf8();
        }

        text.byte_to_point(byte)
    }
}

pub struct NextWord;

impl Motion for NextWord {
    fn motion(&mut self, text: &dyn AnyText, pos: Point) -> Point {
        let mut byte = text.point_to_byte(pos);
        let chars = text.byte_slice(byte..).chars();

        let is_sep = |c: char| c.is_whitespace() || !c.is_alphanumeric() || c.is_uppercase();

        let mut found_sep = false;
        for c in chars {
            if found_sep && !c.is_whitespace() {
                break;
            }

            if is_sep(c) {
                found_sep = true;
            }

            byte += c.len_utf8();
        }

        text.byte_to_point(byte)
    }
}

/// Whitespace delimited word
pub struct NextToken;

impl Motion for NextToken {
    fn motion(&mut self, text: &dyn AnyText, pos: Point) -> Point {
        let mut byte = text.point_to_byte(pos);
        let chars = text.byte_slice(byte..).chars();

        let mut found_whitespace = false;
        for c in chars {
            if found_whitespace && !c.is_whitespace() {
                break;
            }

            if c.is_whitespace() {
                found_whitespace = true;
            }

            byte += c.len_utf8();
        }

        text.byte_to_point(byte)
    }
}
