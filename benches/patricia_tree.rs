use criterion::{black_box, criterion_group, criterion_main, Bencher, Criterion};
use patricia_tree::PatriciaTree;
use rand::{distributions::Standard, prelude::*};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("PatriciaTree<i32>::get() 1k", bench_get::<i32, 1_000>);
    c.bench_function("PatriciaTree<i32>::get() 10k", bench_get::<i32, 10_000>);
    c.bench_function("PatriciaTree<i32>::get() 100k", bench_get::<i32, 100_000>);
    c.bench_function("PatriciaTree<i32>::get() 1M", bench_get::<i32, 1_000_000>);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

fn bench_get<T, const N: usize>(b: &mut Bencher)
where
    Standard: Distribution<T>,
{
    // Generate a completely random Patricia tree.
    let mut tree = PatriciaTree::<T>::new();
    let mut all_keys = Vec::with_capacity(N);

    while all_keys.len() < N {
        let key = random::<[u8; 32]>();
        let value = random::<T>();

        if tree.insert(&key, value).is_none() {
            all_keys.push(key);
        }
    }

    // Run the benchmark.
    let mut key_iter = all_keys.iter().cycle();
    b.iter(|| black_box(tree.get(key_iter.next().unwrap())));
}
