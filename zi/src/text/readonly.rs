use core::fmt;
use std::fs::File;
use std::ops::{self, Deref};
use std::path::Path;
use std::sync::OnceLock;
use std::{io, str};

use memmap2::{Mmap, MmapOptions};

use crate::text::{AnyTextMut, Text, TextBase};

/// A readonly text buffer suitable for reading large files incrementally.
pub struct ReadonlyText<B> {
    buf: B,
    len_lines: OnceLock<usize>,
}

impl<B: Deref<Target = [u8]>> ReadonlyText<B> {
    pub fn new(buf: B) -> Self {
        str::from_utf8(&buf).expect("readonly text implementation only supports utf-8");
        Self { buf, len_lines: OnceLock::new() }
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
        Ok(ReadonlyText { buf, len_lines: OnceLock::new() })
    }
}

impl<B: Deref<Target = [u8]>> fmt::Display for ReadonlyText<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        String::from_utf8_lossy(&self.buf).fmt(f)
    }
}

impl<B: Deref<Target = [u8]>> fmt::Debug for ReadonlyText<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<B: Deref<Target = [u8]>> Text for ReadonlyText<B> {
    type Slice<'a> = &'a str where Self: 'a;

    fn byte_slice(&self, byte_range: impl ops::RangeBounds<usize>) -> Self::Slice<'_> {
        self.as_str().byte_slice(byte_range)
    }

    fn line_slice(&self, line_range: impl ops::RangeBounds<usize>) -> Self::Slice<'_> {
        self.as_str().line_slice(line_range)
    }

    #[inline]
    fn lines(&self) -> impl DoubleEndedIterator<Item = Self::Slice<'_>> {
        <str as Text>::lines(self.as_str())
    }

    #[inline]
    fn get_line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.as_str().get_line(line_idx)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        <str as Text>::chars(self.as_str())
    }
}

impl<B: Deref<Target = [u8]>> TextBase for ReadonlyText<B> {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }

    #[inline]
    fn len_lines(&self) -> usize {
        *self.len_lines.get_or_init(|| self.as_str().len_lines())
    }

    #[inline]
    fn len_bytes(&self) -> usize {
        self.buf.len()
    }

    #[inline]
    fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.as_str().byte_to_line(byte_idx)
    }

    #[inline]
    fn line_to_byte(&self, line_idx: usize) -> usize {
        self.as_str().line_to_byte(line_idx)
    }

    #[inline]
    fn get_char(&self, byte_idx: usize) -> Option<char> {
        self.as_str().get_char(byte_idx)
    }
}
