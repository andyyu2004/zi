use std::fmt;
use std::path::{Path, PathBuf};

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

impl Display {
    #[inline]
    pub fn into_inner(self) -> PathBuf {
        self.0
    }
}

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

impl AsRef<Path> for Display {
    #[inline]
    fn as_ref(&self) -> &Path {
        &self.0
    }
}
