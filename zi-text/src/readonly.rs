use core::fmt;
use std::fs::File;
use std::io::Read;
use std::ops::{self, Deref};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::{io, str};

use memmap2::{Mmap, MmapOptions};

use crate::{AnyTextMut, Text, TextBase};

/// A readonly text buffer suitable for reading large files incrementally.
pub struct ReadonlyText<B> {
    inner: Arc<Inner<B>>,
}

impl<B> Clone for ReadonlyText<B> {
    #[inline]
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

struct Inner<B> {
    buf: B,
    len_lines: OnceLock<usize>,
    len_utf16_cu: OnceLock<usize>,
}

impl<B: Deref<Target = [u8]>> ReadonlyText<B> {
    pub fn new(buf: B) -> Self {
        str::from_utf8(&buf).expect("readonly text implementation only supports utf-8");
        Self {
            inner: Arc::new(Inner {
                buf,
                len_lines: OnceLock::new(),
                len_utf16_cu: OnceLock::new(),
            }),
        }
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
        unsafe { str::from_utf8_unchecked(&self.inner.buf) }
    }
}

impl ReadonlyText<Mmap> {
    // TODO
    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(path)?;
        let buf = unsafe { MmapOptions::new().map(&file)? };
        Ok(ReadonlyText::new(buf))
    }
}

impl<B: Deref<Target = [u8]>> fmt::Display for ReadonlyText<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        std::str::from_utf8(&self.inner.buf).unwrap().fmt(f)
    }
}

impl<B: Deref<Target = [u8]>> fmt::Debug for ReadonlyText<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<B: Deref<Target = [u8]> + Send + Sync> Text for ReadonlyText<B> {
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
    fn line(&self, line_idx: usize) -> Option<Self::Slice<'_>> {
        self.as_str().line(line_idx)
    }

    #[inline]
    fn chars(&self) -> impl DoubleEndedIterator<Item = char> {
        <str as Text>::chars(self.as_str())
    }

    #[inline]
    fn reader(&self) -> impl Read + '_ {
        <str as Text>::reader(self.as_str())
    }
}

impl<B: Deref<Target = [u8]> + Send + Sync> TextBase for ReadonlyText<B> {
    #[inline]
    fn as_text_mut(&mut self) -> Option<&mut dyn AnyTextMut> {
        None
    }

    #[inline]
    fn len_lines(&self) -> usize {
        *self.inner.len_lines.get_or_init(|| self.as_str().len_lines())
    }

    #[inline]
    fn len_utf16_cu(&self) -> usize {
        *self.inner.len_utf16_cu.get_or_init(|| self.as_str().len_utf16_cu())
    }

    #[inline]
    fn len_bytes(&self) -> usize {
        self.inner.buf.len()
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

    #[inline]
    fn byte_to_utf16_cu(&self, byte_idx: usize) -> usize {
        self.as_str().byte_to_utf16_cu(byte_idx)
    }

    #[inline]
    fn utf16_cu_to_byte(&self, cu_idx: usize) -> usize {
        self.as_str().utf16_cu_to_byte(cu_idx)
    }
}
