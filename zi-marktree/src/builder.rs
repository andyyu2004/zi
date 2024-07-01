use std::collections::BTreeMap;
use std::marker::PhantomData;

use arrayvec::ArrayVec;
use crop::tree::{Arc, Inode, Lnode, Node, Tree};
use tinyset::SetU64;

use crate::key::{Flags, Key};
use crate::{Bias, Extent, Inserter, Leaf, MarkTree, MarkTreeId, ARITY};

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

impl<Id: MarkTreeId, const N: usize> FromIterator<(Id, MarkBuilder)> for MarkTree<Id, N> {
    fn from_iter<T: IntoIterator<Item = (Id, MarkBuilder)>>(iter: T) -> Self {
        let mut map = BTreeMap::new();
        for (id, builder) in iter {
            let id = id.into();
            let start_key = Key::new(id, builder.start_flags);
            map.entry(builder.at).or_insert_with(SetU64::new).insert(start_key.into_raw());

            let end_key = Key::new(id, builder.end_flags | Flags::END);
            map.entry(builder.at + builder.width)
                .or_insert_with(SetU64::new)
                .insert(end_key.into_raw());
        }

        let insertions = map.into_iter().collect::<Vec<_>>();
        let root = from_sorted(0, &insertions);
        let tree = Tree::from(root);
        Self { tree, _id: PhantomData }
    }
}

fn from_sorted<const N: usize>(
    mut offset: usize,
    inputs: &[(usize, SetU64)],
) -> Arc<Node<ARITY, Leaf<N>>> {
    if inputs.len() < N {
        let mut extents = ArrayVec::<Extent, N>::new();

        for &(at, ref keys) in inputs {
            extents.push(Extent { length: at - offset, keys: keys.clone() });
            offset = at;
        }

        return Arc::new(Node::Leaf(Lnode::from(Leaf { extents })));
    }

    // Split into ARITY number of chunks and recurse
    let chunk_size =
        if inputs.len() % ARITY == 0 { inputs.len() / ARITY } else { 1 + inputs.len() / ARITY };

    Arc::new(Node::Internal(Inode::from_children(inputs.chunks(chunk_size).map(|chunk| {
        let node = from_sorted(offset, chunk);
        offset += node.summary().bytes;
        node
    }))))
}
