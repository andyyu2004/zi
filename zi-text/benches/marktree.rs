use divan::Bencher;
use zi_text::MarkTree;

// #[global_allocator]
// static GLOBAL: divan::AllocProfiler<mimalloc::MiMalloc> =
//     divan::AllocProfiler::new(mimalloc::MiMalloc);
//
// thread 'rustc' panicked at /rustc/3cb521a4344f0b556b81c55eec8facddeb1ead83/compiler/rustc_data_structures/src/sync.rs:338:42:
// assertion failed: *old == value

fn main() {
    divan::main();
}

#[divan::bench]
fn bench_marktree_insert(bencher: Bencher<'_, '_>) {
    bencher.bench_local(move || {
        let mut tree = MarkTree::<_, 8>::new(100_000);
        (0..2_000).for_each(|i| tree.insert((i, i as u64)));
    });
}

#[divan::bench]
fn bench_marktree_get(bencher: Bencher<'_, '_>) {
    let mut tree = MarkTree::<_, 8>::new(100_000);
    (0..20000).for_each(|i| tree.insert((i, i as u64)));

    bencher.bench_local(move || {
        (0..10000).for_each(|i| {
            let item = tree.get(i as u64);
            assert_eq!(item, Some((i, i as u64)));
        });
    });
}
