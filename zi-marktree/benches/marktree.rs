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

impl From<Id> for u64 {
    #[inline]
    fn from(id: Id) -> u64 {
        id.0 as u64
    }
}

impl From<u64> for Id {
    #[inline]
    fn from(id: u64) -> Id {
        Id(id as usize)
    }
}

impl MarkTreeId for Id {}

// Get performance is a lot worse with larger leaf sizes.
const LEAF_SIZES: [usize; 3] = [16, 32, 64];

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
            MarkTree::<Id, LEAF_SIZE>::build(
                100_000,
                (0..2000).map(|i| (Id(i), MarkBuilder::new(i).width(k))),
            )
        })
        .bench_local_values(|mut tree| {
            (0..1_000).for_each(|i| assert_eq!(tree.delete(Id(i)), Some(i..i + k)));
        });
}

#[divan::bench(consts = LEAF_SIZES, args = [10, 1_000, 10_000])]
fn bench_marktree_drain<const LEAF_SIZE: usize>(bencher: Bencher<'_, '_>, len: usize) {
    bencher
        .with_inputs(|| {
            MarkTree::<Id, LEAF_SIZE>::build(
                100_000,
                (0..20_000).map(|i| (Id(i), MarkBuilder::new(i).width(i % 8))),
            )
        })
        .bench_local_values(|mut tree| {
            assert_eq!(tree.drain(5_000..5_000 + len).count(), len);
            tree
        });
}

#[divan::bench(consts = LEAF_SIZES)]
fn bench_marktree_get_range<const LEAF_SIZE: usize>(bencher: Bencher<'_, '_>) {
    let k = 4;
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

#[divan::bench(consts = LEAF_SIZES)]
fn bench_marktree_range<const LEAF_SIZE: usize>(bencher: Bencher<'_, '_>) {
    let tree = MarkTree::<Id, LEAF_SIZE>::build(
        100_000,
        (0..20_000).map(|i| (Id(i), MarkBuilder::new(i * 4).width(i % 8))),
    );

    bencher.bench_local(move || {
        (0..100).for_each(|i| {
            let start = i * 500;
            assert_eq!(tree.range(start..start + 2000).count(), 500);
        });
    });
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

#[divan::bench(consts = LEAF_SIZES)]
fn bench_marktree_shift<const LEAF_SIZE: usize>(bencher: Bencher<'_, '_>) {
    bencher
        .with_inputs(|| {
            MarkTree::<Id, LEAF_SIZE>::build(
                100_000,
                (0..10_000).map(|i| (Id(i), MarkBuilder::new(i * 10).width(4))),
            )
        })
        .bench_local_values(|mut tree| {
            (0..1_000).for_each(|i| tree.shift(i * 50..=i * 50, 2));
        });
}
