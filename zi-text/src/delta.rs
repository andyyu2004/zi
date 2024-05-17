use std::borrow::Cow;
use std::ops::RangeBounds;
use std::{fmt, ops};

use stdx::range::RangeExt;

use super::Text;

// A set of text deltas to apply to a document.
// This has the same semantics as the lsp `TextEdit[]`.
// See https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/#textEditArray
#[derive(Clone, Default, Debug)]
pub struct Deltas<'a> {
    /// The set of deltas to apply to the text stored order by their start point ascending.
    deltas: Box<[Delta<'a>]>,
}

impl<'a> Deltas<'a> {
    /// See [`Deltas`] for more information.
    /// This will sort the deltas by their start point, and assert they are non-overlapping, and use the same unit of range.
    #[must_use]
    pub fn new(deltas: impl IntoIterator<Item = Delta<'a>>) -> Self {
        let mut deltas = deltas.into_iter().collect::<Box<_>>();
        deltas.sort_by(|a, b| {
            a.range()
                .start
                .partial_cmp(&b.range().start)
                .expect("cannot mix point and byte ranges within set of deltas")
        });

        deltas.iter().zip(deltas.iter().skip(1)).for_each(|(a, b)| {
            assert!(
                !a.range().intersects(&b.range()),
                "deltas must not overlap: {:?} and {:?}",
                a,
                b
            );
        });

        Self { deltas }
    }

    /// Returns an iterator over the deltas ordered by their start point descending.
    pub fn iter(&self) -> impl Iterator<Item = &Delta<'a>> {
        self.deltas.iter().rev()
    }

    pub(crate) fn apply(&self, text: &mut impl TextReplace) -> Deltas<'static> {
        let mut inverse_deltas = Vec::<Delta<'_>>::with_capacity(self.deltas.len());

        // Reminder that this iterator returns deltas are sorted by their start point in descending order.
        for delta in self.iter() {
            let shift = delta.text().len() as isize - delta.range().len() as isize;
            inverse_deltas.iter_mut().for_each(|d| d.shift(shift));

            let inverse = delta.apply(text);
            inverse_deltas.push(inverse);
        }

        Deltas::new(inverse_deltas)
    }

    pub fn has_inserts(&self) -> bool {
        self.deltas.iter().any(|d| !d.text().is_empty())
    }

    pub fn is_identity(&self) -> bool {
        // There are more cases where a delta is an identity,
        self.deltas.iter().all(|d| d.is_identity())
    }

    pub fn insert_at(at: usize, text: impl Into<Cow<'a, str>>) -> Self {
        Deltas::new([Delta::insert_at(at, text)])
    }

    pub fn delete(range: impl Into<DeltaRange>) -> Self {
        Deltas::new([Delta::delete(range)])
    }

    pub fn to_owned(&self) -> Deltas<'static> {
        Deltas::new(self.deltas.iter().map(|d| d.to_owned()))
    }
}

#[derive(Clone)]
pub struct Delta<'a> {
    /// The range to replace
    range: DeltaRange,
    /// The text to replace the range with
    text: Cow<'a, str>,
}

impl fmt::Debug for Delta<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} => {:?}", self.range, self.text)
    }
}

pub type DeltaRange = ops::Range<usize>;

impl<'a> Delta<'a> {
    pub fn new(range: impl Into<DeltaRange>, text: impl Into<Cow<'a, str>>) -> Self {
        Self { range: range.into(), text: text.into() }
    }

    #[inline]
    pub fn delete(range: impl Into<DeltaRange>) -> Self {
        Self::new(range, "")
    }

    #[inline]
    pub fn insert_at(at: usize, text: impl Into<Cow<'a, str>>) -> Self {
        Self::new(at..at, text)
    }

    #[inline]
    pub fn range(&self) -> DeltaRange {
        self.range.clone()
    }

    #[inline]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[inline]
    pub fn to_owned(&self) -> Delta<'static> {
        Delta::new(self.range.clone(), self.text.to_string())
    }

    #[inline]
    pub fn is_identity(&self) -> bool {
        self.text.is_empty() && self.range.is_empty()
    }

    /// Apply the delta to the text and return the inverse delta
    pub(crate) fn apply(&self, text: &mut impl TextReplace) -> Delta<'static> {
        let byte_range = self.range();
        let start = byte_range.start;
        let deleted_text = text.byte_slice(byte_range.clone()).to_string();
        text.replace(byte_range, self.text());
        Delta::new(start..start + self.text.len(), deleted_text)
    }

    fn shift(&mut self, shift: isize) {
        self.range = (self.range.start as isize + shift).try_into().unwrap()
            ..(self.range.end as isize + shift).try_into().unwrap();
    }
}

// HACK trait, do not expose
pub(crate) trait TextReplace: Text {
    fn replace(&mut self, byte_range: impl RangeBounds<usize>, text: &str);
}

impl TextReplace for String {
    fn replace(&mut self, byte_range: impl RangeBounds<usize>, text: &str) {
        self.replace_range(byte_range, text);
    }
}

impl TextReplace for crop::Rope {
    fn replace(&mut self, byte_range: impl RangeBounds<usize>, text: &str) {
        self.replace(byte_range, text);
    }
}
