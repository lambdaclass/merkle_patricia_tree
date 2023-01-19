use std::time::Duration;

use self::common::{bench_compute_hash, bench_get, bench_insert};
use criterion::{criterion_group, criterion_main, Criterion};
use sha3::Keccak256;

mod common;
mod parity;

fn criterion_benchmark(c: &mut Criterion) {
    c.benchmark_group("root_hash_keccak256")
        .measurement_time(Duration::from_secs(10))
        .bench_function("100", bench_compute_hash::<100, Keccak256>())
        .bench_function("500", bench_compute_hash::<500, Keccak256>())
        .bench_function("1k", bench_compute_hash::<1000, Keccak256>())
        .bench_function("2k", bench_compute_hash::<2000, Keccak256>())
        .bench_function("5k", bench_compute_hash::<5000, Keccak256>())
        .bench_function("10k", bench_compute_hash::<10000, Keccak256>());

    c.benchmark_group("PatriciaMerkleTree<Vec<u8>, &[u8], Keccak256>::get()")
        .bench_function("1k", bench_get::<1_000>())
        .bench_function("10k", bench_get::<10_000>())
        .bench_function("100k", bench_get::<100_000>())
        .bench_function("1M", bench_get::<1_000_000>());

    c.benchmark_group("PatriciaMerkleTree<Vec<u8>, &[u8], Keccak256>::insert()")
        .bench_function("1k", bench_insert::<1_000>())
        .bench_function("10k", bench_insert::<10_000>())
        .bench_function("100k", bench_insert::<100_000>())
        .bench_function("1M", bench_insert::<1_000_000>());

    c.benchmark_group("parity get()")
        .bench_function(
            "1k",
            parity::bench_get::<
                1_000,
                reference_trie::ExtensionLayout,
                reference_trie::ReferenceTrieStream,
            >(),
        )
        .bench_function(
            "10k",
            parity::bench_get::<
                10_000,
                reference_trie::ExtensionLayout,
                reference_trie::ReferenceTrieStream,
            >(),
        )
        .bench_function(
            "100k",
            parity::bench_get::<
                100_000,
                reference_trie::ExtensionLayout,
                reference_trie::ReferenceTrieStream,
            >(),
        )
        .bench_function(
            "1m",
            parity::bench_get::<
                1_000_000,
                reference_trie::ExtensionLayout,
                reference_trie::ReferenceTrieStream,
            >(),
        );

    c.benchmark_group("parity insert()")
        .bench_function(
            "1k",
            parity::bench_insert::<
                1_000,
                reference_trie::ExtensionLayout,
                reference_trie::ReferenceTrieStream,
            >(),
        )
        .bench_function(
            "10k",
            parity::bench_insert::<
                10_000,
                reference_trie::ExtensionLayout,
                reference_trie::ReferenceTrieStream,
            >(),
        )
        .bench_function(
            "100k",
            parity::bench_insert::<
                100_000,
                reference_trie::ExtensionLayout,
                reference_trie::ReferenceTrieStream,
            >(),
        )
        .bench_function(
            "1m",
            parity::bench_insert::<
                1_000_000,
                reference_trie::ExtensionLayout,
                reference_trie::ReferenceTrieStream,
            >(),
        );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
