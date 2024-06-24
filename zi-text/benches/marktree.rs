use divan::Bencher;
use zi_text::MarkTree;

#[global_allocator]
static GLOBAL: divan::AllocProfiler<mimalloc::MiMalloc> =
    divan::AllocProfiler::new(mimalloc::MiMalloc);

fn main() {
    divan::main();
}

#[divan::bench]
fn bench_marktree_insert(bencher: Bencher<'_, '_>) {
    bencher.bench_local(move || {
        let mut tree = MarkTree::<_, 8>::new(100_000);
        (0..2_000).for_each(|i| tree.insert(i, i as u64));
    });
}

#[divan::bench]
fn bench_marktree_delete(bencher: Bencher<'_, '_>) {
    bencher
        .with_inputs(|| {
            let mut tree = MarkTree::<_, 8>::new(100_000);
            (0..2_000).for_each(|i| tree.insert(i, i as u64));
            tree
        })
        .bench_local_values(|mut tree| {
            (0..1_000).for_each(|i| assert_eq!(tree.delete(i as u64), Some((i, i as u64))))
        })
}

#[divan::bench]
fn bench_marktree_get(bencher: Bencher<'_, '_>) {
    let mut tree = MarkTree::<_, 8>::new(100_000);
    (0..20000).for_each(|i| tree.insert(i, i as u64));

    bencher.bench_local(move || {
        (0..10000).for_each(|i| {
            let item = tree.get(i as u64);
            assert_eq!(item, Some((i, i as u64)));
        });
    });
}
