use std::collections::BTreeMap;
use std::iter;
use std::marker::PhantomData;

use arrayvec::ArrayVec;
use crop::tree::TreeBuilder;
use tinyset::SetU64;

use crate::key::{Flags, Key};
use crate::{Bias, Extent, Inserter, Leaf, MarkTree, MarkTreeId};

#[derive(Debug, Clone, Copy)]
pub struct MarkBuilder {
    pub(super) at: usize,
    pub(super) width: usize,
    pub(super) start_flags: Flags,
    pub(super) end_flags: Flags,
}

impl MarkBuilder {
    pub fn new(at: usize) -> Self {
        Self { at, width: 0, start_flags: Flags::empty(), end_flags: Flags::END }
    }

    pub fn insert<Id: MarkTreeId, const N: usize>(self, tree: &mut MarkTree<Id, N>, id: Id) {
        drop(Inserter { tree, id, builder: self })
    }

    pub fn width(mut self, width: usize) -> Self {
        self.width = width;
        if width > 0 {
            self.start_flags.insert(Flags::RANGE);
            self.end_flags.insert(Flags::RANGE);
        } else {
            self.start_flags.remove(Flags::RANGE);
            self.end_flags.remove(Flags::RANGE);
        }
        self
    }

    pub fn start_bias(mut self, bias: Bias) -> Self {
        match bias {
            Bias::Left => self.start_flags.insert(Flags::BIAS_LEFT),
            Bias::Right => self.start_flags.remove(Flags::BIAS_LEFT),
        }
        self
    }

    pub fn end_bias(mut self, bias: Bias) -> Self {
        match bias {
            Bias::Left => self.end_flags.insert(Flags::BIAS_LEFT),
            Bias::Right => self.end_flags.remove(Flags::BIAS_LEFT),
        }
        self
    }
}

impl<Id: MarkTreeId, const N: usize> MarkTree<Id, N> {
    /// Fast construction of a mark tree from a iterator of `(id, mark)` pairs.
    /// The `len` must be larger than any position of the marks.
    pub fn build<T: IntoIterator<Item = (Id, MarkBuilder)>>(len: usize, iter: T) -> Self {
        let mut iter = iter.into_iter().peekable();
        if iter.peek().is_none() {
            return Self::new(len);
        }

        let mut map = BTreeMap::new();
        for (id, builder) in iter {
            let id = id.into();
            let start_key = Key::new(id, builder.start_flags);
            map.entry(builder.at).or_insert_with(SetU64::new).insert(start_key.into_raw());

            if builder.width > 0 {
                let end_key = Key::new(id, builder.end_flags | Flags::END);
                map.entry(builder.at + builder.width)
                    .or_insert_with(SetU64::new)
                    .insert(end_key.into_raw());
            }
        }

        let mut extents = vec![];
        let (&offset, _) = map.first_key_value().expect("insertions is non-empty");
        if offset > 0 {
            extents.push(Extent { length: offset, keys: SetU64::new() });
        }

        let next_offsets = map
            .iter()
            .map(|(&offset, _)| offset)
            .skip(1)
            .chain(iter::once(len))
            .collect::<Vec<_>>();

        assert_eq!(next_offsets.len(), map.len());
        extents.extend(map.into_iter().zip(next_offsets).map(
            |((start_offset, keys), end_offset)| Extent { length: end_offset - start_offset, keys },
        ));

        let mut builder = TreeBuilder::new();
        let mut chunks = extents.into_iter().array_chunks::<N>();
        for leaf in chunks.by_ref().map(ArrayVec::from).map(Leaf::from) {
            builder.append(leaf);
        }

        if let Some(rem) = chunks.into_remainder() {
            let array = ArrayVec::from_iter(rem);
            builder.append(Leaf::from(array));
        }

        let tree = Self { tree: builder.build(), _id: PhantomData };

        assert_eq!(tree.len(), len);

        tree
    }
}
