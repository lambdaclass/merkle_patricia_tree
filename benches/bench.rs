use criterion::{black_box, criterion_group, criterion_main, BatchSize, Bencher, Criterion};
use patricia_merkle_tree::{NibbleIterator, PatriciaMerkleTree, TreePath};
use rand::{distributions::Uniform, prelude::Distribution, thread_rng, RngCore};
use sha3::Keccak256;
use std::{io, iter::Copied, slice::Iter};

fn criterion_benchmark(c: &mut Criterion) {
    c.benchmark_group("PatriciaMerkleTree::<MyNode, Keccak256>::get()")
        .bench_function("1k", bench_get::<1_000>())
        .bench_function("10k", bench_get::<10_000>())
        .bench_function("100k", bench_get::<100_000>())
        .bench_function("1M", bench_get::<1_000_000>());

    c.benchmark_group("PatriciaMerkleTree::<MyNode, Keccak256>::insert()")
        .bench_function("1k", bench_insert::<1_000>())
        .bench_function("10k", bench_insert::<10_000>())
        .bench_function("100k", bench_insert::<100_000>())
        .bench_function("1M", bench_insert::<1_000_000>());
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

#[derive(Clone, Debug, Eq, PartialEq)]
struct MyNodePath(Vec<u8>);

impl TreePath for MyNodePath {
    type Iterator<'a> = NibbleIterator<Copied<Iter<'a, u8>>>;

    fn encode(&self, mut target: impl io::Write) -> io::Result<()> {
        target.write_all(self.0.as_ref())
    }

    fn encoded_iter(&self) -> Self::Iterator<'_> {
        NibbleIterator::new(self.0.iter().copied())
    }
}

fn bench_get<const N: usize>() -> impl FnMut(&mut Bencher) {
    // Generate a completely random Patricia Merkle tree.
    let mut tree = PatriciaMerkleTree::<MyNodePath, i32, Keccak256>::new();
    let mut all_paths = Vec::with_capacity(N);

    let mut rng = thread_rng();
    let distr = Uniform::from(16..=64);

    while all_paths.len() < N {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        if tree
            .insert(MyNodePath(path.clone()), rng.next_u32() as i32)
            .is_none()
        {
            all_paths.push(MyNodePath(path));
        }
    }

    move |b| {
        let mut path_iter = all_paths.iter().cycle();
        b.iter(|| black_box(tree.get(path_iter.next().unwrap())));
    }
}

fn bench_insert<const N: usize>() -> impl FnMut(&mut Bencher) {
    // Generate a completely random Patricia Merkle tree.
    let mut tree = PatriciaMerkleTree::<MyNodePath, i32, Keccak256>::new();
    let mut all_paths = Vec::with_capacity(N);

    let mut rng = thread_rng();
    let distr = Uniform::from(16..=64);

    while all_paths.len() < N {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        if tree
            .insert(MyNodePath(path.clone()), rng.next_u32() as i32)
            .is_none()
        {
            all_paths.push(MyNodePath(path));
        }
    }

    // Generate random nodes to insert.
    let mut new_nodes = Vec::new();
    while new_nodes.len() < 1000 {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        let path = MyNodePath(path);
        if tree.get(&path).is_none() {
            new_nodes.push((path, rng.next_u32() as i32));
        }
    }

    move |b| {
        let mut path_iter = new_nodes.iter().cycle();
        b.iter_batched_ref(
            || (tree.clone(), Some(path_iter.next().unwrap().clone())),
            |(tree, node)| {
                let (path, value) = node.take().unwrap();
                black_box(tree.insert(path, value));
            },
            BatchSize::LargeInput,
        )
    }
}
