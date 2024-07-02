use divan::Bencher;
use zi_marktree::{MarkBuilder, MarkTree, MarkTreeId};

#[global_allocator]
static GLOBAL: divan::AllocProfiler<mimalloc::MiMalloc> =
    divan::AllocProfiler::new(mimalloc::MiMalloc);

fn main() {
    divan::main();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Id(usize);

impl From<Id> for u32 {
    #[inline]
    fn from(id: Id) -> u32 {
        debug_assert!(id.0 < u32::MAX as usize);
        id.0 as u32
    }
}

impl From<u32> for Id {
    #[inline]
    fn from(id: u32) -> Id {
        Id(id as usize)
    }
}

impl MarkTreeId for Id {}

const LEAF_SIZES: [usize; 7] = [4, 8, 16, 32, 64, 128, 256];

#[divan::bench(consts = LEAF_SIZES)]
fn bench_marktree_insert<const LEAF_SIZE: usize>(bencher: Bencher<'_, '_>) {
    bencher.bench_local(move || {
        let mut tree = MarkTree::<Id, LEAF_SIZE>::new(100_000);
        (0..2_000).for_each(|i| drop(tree.insert(i, Id(i))));
    });
}

#[divan::bench(consts = LEAF_SIZES)]
fn bench_marktree_build<const LEAF_SIZE: usize>(bencher: Bencher<'_, '_>) {
    bencher.bench_local(move || {
        MarkTree::<Id, LEAF_SIZE>::build(
            100_000,
            (0..20_000).map(|i| (Id(i), MarkBuilder::new(i).width(i))),
        );
    });
}

#[divan::bench(consts = LEAF_SIZES)]
fn bench_marktree_delete<const LEAF_SIZE: usize>(bencher: Bencher<'_, '_>) {
    // FIXME this is really slow if k != 0 for some reason.
    let k = 0;
    bencher
        .with_inputs(|| {
            let tree = MarkTree::<Id, LEAF_SIZE>::build(
                100_000,
                (0..2000).map(|i| (Id(i), MarkBuilder::new(i).width(k))),
            );
            tree
        })
        .bench_local_values(|mut tree| {
            (0..1_000).for_each(|i| assert_eq!(tree.delete(Id(i)), Some(i..i + k)))
        })
}

#[divan::bench(consts = LEAF_SIZES)]
fn bench_marktree_get<const LEAF_SIZE: usize>(bencher: Bencher<'_, '_>) {
    // FIXME this is really slow if k != 0 for some reason.
    let k = 0;
    let tree = MarkTree::<Id, LEAF_SIZE>::build(
        100_000,
        (0..20_000).map(|i| (Id(i), MarkBuilder::new(i).width(k))),
    );

    bencher.bench_local(move || {
        (0..10000).for_each(|i| {
            let item = tree.get(Id(i));
            assert_eq!(item, Some(i..i + k));
        });
    });
}
