use std::collections::BTreeMap;
use std::marker::PhantomData;

use arrayvec::ArrayVec;
use crop::tree::{Arc, Inode, Lnode, Node, TreeBuilder};
use tinyset::SetU64;

use crate::key::{Flags, Key};
use crate::{Extent, Insertion, Leaf, MarkTree, MarkTreeId, ARITY};

pub struct Builder<const N: usize> {
    builder: TreeBuilder<ARITY, Leaf<N>>,
}

// not sure how to make this work
impl<const N: usize> Builder<N> {
    pub fn new() -> Self {
        Self { builder: TreeBuilder::new() }
    }

    pub fn build<Id: MarkTreeId>(self) -> MarkTree<Id, N> {
        let tree = self.builder.build();
        MarkTree::<Id, N> { tree, _id: PhantomData }
    }
}

fn from_iter<const N: usize>(
    iter: impl IntoIterator<Item = Insertion>,
) -> Arc<Node<ARITY, Leaf<N>>> {
    let mut map = BTreeMap::new();
    for ins in iter {
        let start_key = Key::new(ins.id, ins.start_flags);
        map.entry(ins.at).or_insert_with(SetU64::new).insert(start_key.into_raw());

        let end_key = Key::new(ins.id, ins.end_flags | Flags::END);
        map.entry(ins.at + ins.width).or_insert_with(SetU64::new).insert(end_key.into_raw());
    }

    let insertions = map.into_iter().collect::<Vec<_>>();
    from_sorted(0, &insertions)
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
