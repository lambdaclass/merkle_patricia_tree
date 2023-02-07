use criterion::{black_box, Bencher};
use digest::Digest;
use patricia_merkle_tree::PatriciaMerkleTree;
use rand::{distributions::Uniform, prelude::Distribution, thread_rng, RngCore};
use sha3::Keccak256;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub fn bench_get<const N: usize>() -> impl FnMut(&mut Bencher) {
    // Generate a completely random Patricia Merkle tree.
    let mut tree = PatriciaMerkleTree::<Vec<u8>, &[u8; 32], Keccak256>::new();
    let mut all_paths = Vec::with_capacity(N);

    let value = &[0; 32];

    let mut rng = thread_rng();
    let distr = Uniform::from(16..=64);

    while all_paths.len() < N {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        if tree.insert(path.clone(), value).is_none() {
            all_paths.push(path);
        }
    }

    move |b| {
        let mut path_iter = all_paths.iter().cycle();
        b.iter(|| tree.get(black_box(path_iter.next().unwrap())));
    }
}

pub fn bench_insert<const N: usize>() -> impl FnMut(&mut Bencher) {
    // Generate a completely random Patricia Merkle tree.
    let mut tree = PatriciaMerkleTree::<Vec<u8>, _, Keccak256>::new();
    let mut all_paths = Vec::with_capacity(N);

    let value = &[0; 32];

    let mut rng = thread_rng();
    let distr = Uniform::from(16..=64);

    while all_paths.len() < N {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        if tree.insert(path.clone(), value).is_none() {
            all_paths.push(path);
        }
    }

    // Generate random nodes to insert.
    let mut new_nodes = Vec::new();
    while new_nodes.len() < 1000 {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        if tree.get(&path).is_none() {
            new_nodes.push((path, value));
        }
    }

    move |b| {
        // This (iter_custom) is required because of a bug in criterion, which will include setup
        // time in the final calculation (which we don't want).
        b.iter_custom(|num_iters| {
            const STEP: usize = 1024;

            let mut delta = Duration::ZERO;
            for offset in (0..num_iters).step_by(STEP) {
                let new_nodes = new_nodes.clone();
                let mut tree = tree.clone();

                let mut path_iter = new_nodes.into_iter().cycle();
                tree.reserve_next_power_of_two();

                // To make measurements more effective, values are inserted STEP at a time, making
                // all values except the first one to be inserted with a tree slightly larger than
                // intended. It should not affect the results significantly.
                let measure = Instant::now();
                for _ in offset..num_iters.min(offset + STEP as u64) {
                    let (path, value) = path_iter.next().unwrap();
                    tree.insert(black_box(path), black_box(value));
                }
                delta += measure.elapsed();
            }

            delta
        });
    }
}

pub fn bench_compute_hash<const N: usize, H: Digest + Clone>() -> impl FnMut(&mut Bencher) {
    let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, H>::new();
    let mut all_paths = Vec::with_capacity(N);

    let mut rng = thread_rng();
    let distr = Uniform::from(16..=64);

    while all_paths.len() < N {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        let value_len = distr.sample(&mut rng) as usize;

        let mut value = vec![0; value_len];
        rng.fill_bytes(&mut value);

        if tree.insert(path.clone(), value).is_none() {
            all_paths.push(path);
        }
    }

    move |b| {
        b.iter_custom(|num_iters| {
            let mut delta = Duration::ZERO;
            for _ in 0..num_iters {
                let mut tree = tree.clone();
                let measure = Instant::now();
                black_box(tree.compute_hash());
                delta += measure.elapsed();
            }
            delta
        });
    }
}

pub fn bench_compute_hash_inserts<const N: usize, H: Digest + Clone>() -> impl FnMut(&mut Bencher) {
    let mut rng = thread_rng();
    let distr = Uniform::from(16..=64);
    let mut data = BTreeMap::new();

    while data.len() < N {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        let value_len = distr.sample(&mut rng) as usize;

        let mut value = vec![0; value_len];
        rng.fill_bytes(&mut value);

        data.insert(path, value);
    }

    move |b| {
        let data: Vec<_> = data.clone().into_iter().collect();
        let data: Vec<_> = data
            .iter()
            .map(|x| (x.0.as_slice(), x.1.as_slice()))
            .collect();

        b.iter_custom(|num_iters| {
            let mut delta = Duration::ZERO;
            for _ in 0..num_iters {
                let iter = data.iter();
                let measure = Instant::now();
                let mut tree = PatriciaMerkleTree::<_, _, H>::new();
                for (key, val) in iter {
                    tree.insert(black_box(*key), black_box(*val));
                }
                black_box(tree.compute_hash());
                delta += measure.elapsed();
            }
            delta
        });
    }
}

pub fn bench_compute_hash_sorted<const N: usize, H: Digest + Clone>() -> impl FnMut(&mut Bencher) {
    let mut rng = thread_rng();
    let distr = Uniform::from(16..=64);
    let mut data = BTreeMap::new();

    while data.len() < N {
        let path_len = distr.sample(&mut rng) as usize;

        let mut path = vec![0; path_len];
        rng.fill_bytes(&mut path);

        let value_len = distr.sample(&mut rng) as usize;

        let mut value = vec![0; value_len];
        rng.fill_bytes(&mut value);

        data.insert(path, value);
    }

    move |b| {
        let data: Vec<_> = data.clone().into_iter().collect();
        let data: Vec<_> = data
            .iter()
            .map(|x| (x.0.as_slice(), x.1.as_slice()))
            .collect();

        b.iter_custom(|num_iters| {
            let mut delta = Duration::ZERO;
            for _ in 0..num_iters {
                let iter = data.iter();
                let measure = Instant::now();
                black_box(
                    PatriciaMerkleTree::<_, _, H>::compute_hash_from_sorted_iter(black_box(iter)),
                );
                delta += measure.elapsed();
            }
            delta
        });
    }
}
