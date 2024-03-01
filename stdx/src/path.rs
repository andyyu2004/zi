use std::fmt;
use std::path::PathBuf;

pub trait PathExt {
    fn display_owned(self) -> Display;
}

impl PathExt for PathBuf {
    fn display_owned(self) -> Display {
        Display(self)
    }
}

#[derive(Clone)]
pub struct Display(PathBuf);

impl fmt::Display for Display {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(f)
    }
}

impl fmt::Debug for Display {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
