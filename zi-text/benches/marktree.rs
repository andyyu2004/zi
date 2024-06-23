use divan::Bencher;
use zi_text::MarkTree;

fn main() {
    divan::main();
}

#[divan::bench]
fn bench_marktree_get(bencher: Bencher<'_, '_>) {
    let mut tree = MarkTree::<_, 8>::new(10000);
    (0..4000).for_each(|i| tree.insert((i, i as u64)));
    bencher.bench_local(move || {
        (0..1000).for_each(|i| {
            let item = tree.get(i as u64);
            // assert_eq!(item, Some((i, i as u64)));
        });
    });
}
