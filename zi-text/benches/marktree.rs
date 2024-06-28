use divan::Bencher;
use zi_text::{MarkTree, MarkTreeId};

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

#[divan::bench]
fn bench_marktree_insert(bencher: Bencher<'_, '_>) {
    bencher.bench_local(move || {
        let mut tree = MarkTree::<Id, 8>::new(100_000);
        (0..2_000).for_each(|i| drop(tree.insert(i, Id(i))));
    });
}

#[divan::bench]
fn bench_marktree_delete(bencher: Bencher<'_, '_>) {
    bencher
        .with_inputs(|| {
            let mut tree = MarkTree::<Id, 8>::new(100_000);
            (0..2_000).for_each(|i| drop(tree.insert(i, Id(i))));
            tree
        })
        .bench_local_values(|mut tree| {
            (0..1_000).for_each(|i| assert_eq!(tree.delete(Id(i)), Some(i..i)))
        })
}

#[divan::bench]
fn bench_marktree_get(bencher: Bencher<'_, '_>) {
    let mut tree = MarkTree::<Id, 8>::new(100_000);
    (0..20000).for_each(|i| drop(tree.insert(i, Id(i))));

    bencher.bench_local(move || {
        (0..10000).for_each(|i| {
            let item = tree.get(Id(i));
            assert_eq!(item, Some(i..i));
        });
    });
}
