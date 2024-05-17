use std::fmt;
use std::ops::Deref;

/// Eventually an interned string
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Copy)]
pub struct Symbol(&'static str);

impl Symbol {
    pub(crate) const fn const_new(s: &'static str) -> Self {
        Self(s)
    }
}

impl From<&'static str> for Symbol {
    fn from(s: &'static str) -> Self {
        Self(s)
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for Symbol {
    type Target = str;

    fn deref(&self) -> &'static Self::Target {
        self.0
    }
}
