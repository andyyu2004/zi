pub trait Delimiter {
    const OPEN: char;
    const CLOSE: char;
}

pub trait SymmetricDelimiter: Delimiter {
    const DELIMITER: char = Self::OPEN;
}

pub struct Paren;

impl Delimiter for Paren {
    const OPEN: char = '(';
    const CLOSE: char = ')';
}

pub struct Bracket;

impl Delimiter for Bracket {
    const OPEN: char = '[';
    const CLOSE: char = ']';
}

pub struct Brace;

impl Delimiter for Brace {
    const OPEN: char = '{';
    const CLOSE: char = '}';
}

pub struct Quote;
impl Delimiter for Quote {
    const OPEN: char = '"';
    const CLOSE: char = '"';
}

pub struct Apostrophe;

impl Delimiter for Apostrophe {
    const OPEN: char = '\'';
    const CLOSE: char = '\'';
}

pub struct Backtick;

impl Delimiter for Backtick {
    const OPEN: char = '`';
    const CLOSE: char = '`';
}

pub struct AngleBracket;

impl Delimiter for AngleBracket {
    const OPEN: char = '<';
    const CLOSE: char = '>';
}
