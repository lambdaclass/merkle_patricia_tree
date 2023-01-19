use criterion::{black_box, Bencher};
use memory_db::{HashKey, MemoryDB};
use rand::{distributions::Uniform, prelude::Distribution, thread_rng, RngCore};
use reference_trie::TrieStream;
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};
use trie_db::{NodeCodec, TrieDBMutBuilder, TrieHash, TrieLayout, TrieMut};

pub fn bench_get<L, S, const N: usize>() -> impl FnMut(&mut Bencher)
where
    L: 'static + TrieLayout,
    S: TrieStream,
{
    let mut all_paths = Vec::with_capacity(N);
    let value = &[0; 32];

    let mut rng = thread_rng();
    let distr = Uniform::from(16..=64);

    // TODO: I think having the tree generation code in there causes criterion to include it in the
    //   benchmarked region. Check it.
    move |b| {
        let mut memdb = MemoryDB::<_, HashKey<_>, _>::new(L::Codec::empty_node());
        let mut root = <TrieHash<L>>::default();

        let mut t = TrieDBMutBuilder::<L>::new(&mut memdb, &mut root).build();

        while all_paths.len() < N {
            let path_len = distr.sample(&mut rng) as usize;

            let mut path = vec![0; path_len];
            rng.fill_bytes(&mut path);

            //data.push((path.clone(), value));
            if t.insert(&path, value).unwrap().is_none() {
                all_paths.push(path);
            }
        }
        let mut path_iter = all_paths.iter().cycle();
        b.iter(|| t.get(black_box(path_iter.next().unwrap())));
    }
}

pub fn bench_insert<L, S, const N: usize>() -> impl FnMut(&mut Bencher)
where
    L: 'static + TrieLayout,
    S: TrieStream,
{
    // Generate a completely random Patricia Merkle tree.
    let mut memdb = MemoryDB::<_, HashKey<_>, _>::new(L::Codec::empty_node());
    let mut root = <TrieHash<L>>::default();

    let mut new_nodes = Vec::new();
    {
        let mut tree = TrieDBMutBuilder::<L>::new(&mut memdb, &mut root).build();

        let mut all_paths = Vec::with_capacity(N);
        let value = &[0; 32];

        let mut rng = thread_rng();
        let distr = Uniform::from(16..=32);

        while all_paths.len() < N {
            let path_len = distr.sample(&mut rng) as usize;

            let mut path = vec![0; path_len];
            rng.fill_bytes(&mut path);

            if tree.insert(&path, value).unwrap().is_none() {
                all_paths.push(path);
            }
        }

        // Generate random nodes to insert.
        while new_nodes.len() < 1000 {
            let path_len = distr.sample(&mut rng) as usize;

            let mut path = vec![0; path_len];
            rng.fill_bytes(&mut path);

            if tree.get(&path).unwrap().is_none() {
                new_nodes.push((path, value));
            }
        }
    }

    let memdb = RefCell::new(memdb);
    let root = RefCell::new(root);

    move |b| {
        let mut path_iter = new_nodes.iter().cycle();

        b.iter_custom(|num_iters| {
            const STEP: usize = 1024;

            let mut delta = Duration::ZERO;
            for offset in (0..num_iters).step_by(STEP) {
                let mut memdb = memdb.borrow().clone();
                let mut root = *root.borrow();
                let mut tree = TrieDBMutBuilder::<L>::new(&mut memdb, &mut root).build();

                // To make measurements more effective, values are inserted STEP at a time, making
                // all values except the first one to be inserted with a tree slightly larger than
                // intended. It should not affect the results significantly.
                let measure = Instant::now();
                for _ in offset..num_iters.min(offset + STEP as u64) {
                    let (path, value) = path_iter.next().unwrap().clone();
                    tree.insert(black_box(&path), black_box(value)).unwrap();
                }
                delta += measure.elapsed();
            }

            delta
        });
    }
}
