use std::fmt;
use std::path::{MAIN_SEPARATOR, Path, PathBuf};

pub trait PathExt {
    fn display_owned(self) -> Display;

    fn display_relative_to(self, relative_to: impl AsRef<Path>) -> Relative;
}

impl PathExt for PathBuf {
    fn display_owned(self) -> Display {
        Display(self)
    }

    fn display_relative_to(self, relative_to: impl AsRef<Path>) -> Relative {
        Relative { relative_to: relative_to.as_ref().to_path_buf(), path: self }
    }
}

#[derive(Debug, Clone)]
pub struct Relative {
    relative_to: PathBuf,
    path: PathBuf,
}

impl Relative {
    #[inline]
    pub fn into_inner(self) -> PathBuf {
        self.path
    }
}

impl fmt::Display for Relative {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.path.strip_prefix(&self.relative_to).unwrap_or(&self.path);
        if path == Path::new("") {
            write!(f, ".{MAIN_SEPARATOR}")
        } else if self.path.is_dir() {
            // show trailing slash to indicate directories
            write!(f, "{}{MAIN_SEPARATOR}", path.display())
        } else {
            path.display().fmt(f)
        }
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
