use std::fmt;
use std::sync::Arc;

use nucleo::pattern::{CaseMatching, Normalization};
use nucleo::Nucleo;

use super::*;

// FIXME don't use Debug
pub trait Item: fmt::Debug + Clone + Sync + Send + 'static {}

impl<T> Item for T where T: fmt::Debug + Clone + Sync + Send + 'static {}

/// Wrapper around a `nucleo::Injector`
pub struct Injector<T> {
    injector: nucleo::Injector<T>,
}

impl<T: Item> Injector<T> {
    pub fn new(injector: nucleo::Injector<T>) -> Self {
        Self { injector }
    }

    pub fn push(&self, item: T) {
        self.injector.push(item.clone(), |dst| dst[0] = format!("{item:?}").into());
    }
}

pub struct PickerBuffer<T: Item> {
    id: BufferId,
    text: Rope,
    nucleo: Nucleo<T>,
    end: usize,
}

impl<T: Item> PickerBuffer<T> {
    pub fn new_streamed(id: BufferId) -> (Self, Injector<T>) {
        let nucleo = Nucleo::new(nucleo::Config::DEFAULT, Arc::new(|| {}), None, 1);
        let injector = nucleo.injector();
        (Self { id, text: Rope::new(), nucleo, end: 0 }, Injector::new(injector))
    }

    pub fn new(id: BufferId, options: impl IntoIterator<Item = T>) -> Self {
        let nucleo = Nucleo::new(nucleo::Config::DEFAULT, Arc::new(|| {}), None, 1);
        let injector = Injector::new(nucleo.injector());
        for option in options {
            injector.push(option.clone());
        }

        Self { id, text: Rope::new(), nucleo, end: 0 }
    }
}

impl<T: Item> Buffer for PickerBuffer<T> {
    fn id(&self) -> BufferId {
        self.id
    }

    fn path(&self) -> &Path {
        Path::new("picker")
    }

    fn url(&self) -> Option<Url> {
        None
    }

    fn language_id(&self) -> &FileType {
        &FileType::PICKER
    }

    fn tab_width(&self) -> u8 {
        4
    }

    fn text(&self) -> RopeSlice<'_> {
        self.text.slice(..)
    }

    fn version(&self) -> u32 {
        0
    }

    fn insert_char(&mut self, _pos: Position, c: char) {
        // TODO respect position
        self.text.insert_char(self.end, c);
        self.end += 1;
        self.nucleo.pattern.reparse(
            0,
            &self.writable_text().to_string(),
            CaseMatching::Smart,
            Normalization::Smart,
            false,
        );
    }

    fn writable_range(&self) -> (Bound<usize>, Bound<usize>) {
        // TODO maintain this correctly
        (Bound::Unbounded, Bound::Excluded(self.end))
    }

    fn highlights<'a>(
        &'a self,
        _cursor: &'a mut QueryCursor,
    ) -> Box<dyn Iterator<Item = (Range, HighlightId)> + 'a> {
        // TODO
        Box::new(std::iter::empty())
    }

    fn pre_render(&mut self) {
        self.nucleo.tick(10);
        let snapshot = self.nucleo.snapshot();
        self.text = Rope::from(self.writable_text());
        let mut rope = Rope::new();
        rope.insert(0, "\n-----");
        for item in snapshot.matched_items(..) {
            rope.insert(rope.len_chars(), &format!("\n{:?}", item.data));
        }
        self.text.append(rope);
    }
}
