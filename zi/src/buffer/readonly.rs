use core::fmt;
use std::borrow::Cow;
use std::fs::File;
use std::ops::Deref;
use std::path::Path;
use std::sync::OnceLock;
use std::{io, str};

use memmap2::{Mmap, MmapOptions};
use stdx::iter::BidirectionalIterator;

use super::{LazyText, Text, TextMut};

/// A readonly text buffer suitable for reading large files incrementally.
pub struct ReadonlyText<B> {
    buf: B,
    len_lines: OnceLock<usize>,
    len_chars: OnceLock<usize>,
}

impl<B: Deref<Target = [u8]>> ReadonlyText<B> {
    pub fn new(buf: B) -> Self {
        str::from_utf8(&buf).expect("readonly text implementation only supports utf-8");
        Self { buf, len_lines: OnceLock::new(), len_chars: OnceLock::new() }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.as_ref()
    }
}

impl<B: Deref<Target = [u8]>> AsRef<str> for ReadonlyText<B> {
    #[inline]
    fn as_ref(&self) -> &str {
        // Safety: We've checked that the buffer is valid utf-8 in `new`.
        unsafe { str::from_utf8_unchecked(&self.buf) }
    }
}

impl ReadonlyText<Mmap> {
    pub unsafe fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(path)?;
        let buf = unsafe { MmapOptions::new().map(&file)? };
        Ok(ReadonlyText { buf, len_lines: OnceLock::new(), len_chars: OnceLock::new() })
    }
}

impl<B: Deref<Target = [u8]>> fmt::Display for ReadonlyText<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        String::from_utf8_lossy(&self.buf).fmt(f)
    }
}

/// We intentionally do not implement [`crate::buffer::Text`] for this type because computing
/// those methods would require reading the entire file which we're trying to avoid.
// TODO completely naive implementation that's the same as `str`
// Should maintain an index or something to make this more efficient
impl<B: Deref<Target = [u8]>> LazyText for ReadonlyText<B> {
    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Cow<'_, str>> {
        self.as_str().get_line(line_idx)
    }

    #[inline]
    fn get_char(&self, char_idx: usize) -> Option<char> {
        self.as_str().get_char(char_idx)
    }

    #[inline]
    fn line_to_char(&self, line_idx: usize) -> usize {
        self.as_str().line_to_char(line_idx)
    }

    #[inline]
    fn char_to_line(&self, char_idx: usize) -> usize {
        self.as_str().char_to_line(char_idx)
    }

    #[inline]
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.as_str().char_to_byte(char_idx)
    }

    #[inline]
    fn lines_at(&self, line_idx: usize) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        self.as_str().lines_at(line_idx)
    }

    #[inline]
    fn chars_at(&self, char_idx: usize) -> Box<dyn BidirectionalIterator<Item = char> + '_> {
        self.as_str().chars_at(char_idx)
    }

    #[inline]
    fn chunk_at_byte(&self, byte_idx: usize) -> &str {
        self.as_str().chunk_at_byte(byte_idx)
    }

    #[inline]
    fn byte_slice(&self, range: std::ops::Range<usize>) -> Box<dyn Iterator<Item = &str> + '_> {
        self.as_str().byte_slice(range)
    }

    #[inline]
    fn rev_chars_at_end(&self) -> Box<dyn Iterator<Item = char> + '_> {
        self.as_str().rev_chars_at_end()
    }

    #[inline]
    fn as_text(&self) -> Option<&dyn Text> {
        Some(self)
    }

    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn TextMut> {
        None
    }
}

impl<B: Deref<Target = [u8]>> Text for ReadonlyText<B> {
    #[inline]
    fn len_bytes(&self) -> usize {
        self.buf.len()
    }

    #[inline]
    fn len_lines(&self) -> usize {
        *self.len_lines.get_or_init(|| self.as_str().len_lines())
    }

    #[inline]
    fn len_chars(&self) -> usize {
        *self.len_chars.get_or_init(|| self.as_str().len_chars())
    }
}
