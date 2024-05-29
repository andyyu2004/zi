use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum EditError {
    Readonly,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::Readonly => write!(f, "buffer is readonly"),
        }
    }
}

impl Error for EditError {}
