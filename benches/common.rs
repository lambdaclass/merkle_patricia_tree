use criterion::{black_box, Bencher};
use patricia_merkle_tree::{nibble::NibbleIterator, PatriciaMerkleTree, TreePath};
use rand::{distributions::Uniform, prelude::Distribution, thread_rng, RngCore};
use sha3::Keccak256;
use std::{
    io,
    iter::Copied,
    slice::Iter,
    time::{Duration, Instant},
};

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

pub fn bench_get<const N: usize>() -> impl FnMut(&mut Bencher) {
    // Generate a completely random Patricia Merkle tree.
    let mut tree = PatriciaMerkleTree::<MyNodePath, _, Keccak256>::new();
    let mut all_paths = Vec::with_capacity(N);

    let value = &[0; 32];

    let mut rng = thread_rng();
    let distr = Uniform::from(16..=64);

    while all_paths.len() < N {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        if tree.insert(MyNodePath(path.clone()), value).is_none() {
            all_paths.push(MyNodePath(path));
        }
    }

    move |b| {
        let mut path_iter = all_paths.iter().cycle();
        b.iter(|| black_box(tree.get(path_iter.next().unwrap())));
    }
}

pub fn bench_insert<const N: usize>() -> impl FnMut(&mut Bencher) {
    // Generate a completely random Patricia Merkle tree.
    let mut tree = PatriciaMerkleTree::<MyNodePath, _, Keccak256>::new();
    let mut all_paths = Vec::with_capacity(N);

    let value = &[0; 32];

    let mut rng = thread_rng();
    let distr = Uniform::from(16..=64);

    while all_paths.len() < N {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        if tree.insert(MyNodePath(path.clone()), value).is_none() {
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
            new_nodes.push((path, value));
        }
    }

    move |b| {
        // This (iter_custom) is required because of a bug in criterion, which will include setup
        // time in the final calculation (which we don't want).
        let mut path_iter = new_nodes.iter().cycle();
        b.iter_custom(|num_iters| {
            const STEP: usize = 1024;

            let mut delta = Duration::ZERO;
            for offset in (0..num_iters).step_by(STEP) {
                let mut tree = tree.clone();

                // To make measurements more effective, values are inserted STEP at a time, making
                // all values except the first one to be inserted with a tree slightly larger than
                // intended. It should not affect the results significantly.
                let measure = Instant::now();
                for _ in offset..num_iters.min(offset + STEP as u64) {
                    let (path, value) = path_iter.next().unwrap().clone();
                    tree.insert(path, value);
                }
                delta += measure.elapsed();
            }

            delta
        });
    }
}
