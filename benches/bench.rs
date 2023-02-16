use self::common::{bench_compute_hash, bench_get, bench_insert};
use common::{bench_compute_hash_inserts, bench_compute_hash_sorted};
use criterion::{criterion_group, criterion_main, Criterion};
use sha3::Keccak256;
use std::time::Duration;

mod common;

fn criterion_benchmark(c: &mut Criterion) {
    c.benchmark_group("calculate root keccak256 hash with random items")
        .measurement_time(Duration::from_secs(10))
        .bench_function("100", bench_compute_hash::<100, Keccak256>())
        .bench_function("500", bench_compute_hash::<500, Keccak256>())
        .bench_function("1k", bench_compute_hash::<1000, Keccak256>())
        .bench_function("2k", bench_compute_hash::<2000, Keccak256>())
        .bench_function("5k", bench_compute_hash::<5000, Keccak256>())
        .bench_function("10k", bench_compute_hash::<10000, Keccak256>());

    c.benchmark_group("calculate root keccak256 hash with random items including inserts")
        .measurement_time(Duration::from_secs(10))
        .bench_function("100", bench_compute_hash_inserts::<100, Keccak256>())
        .bench_function("500", bench_compute_hash_inserts::<500, Keccak256>())
        .bench_function("1k", bench_compute_hash_inserts::<1000, Keccak256>())
        .bench_function("2k", bench_compute_hash_inserts::<2000, Keccak256>())
        .bench_function("5k", bench_compute_hash_inserts::<5000, Keccak256>())
        .bench_function("10k", bench_compute_hash_inserts::<10000, Keccak256>());

    c.benchmark_group("calculate root keccak256 hash from sorted random items")
        .measurement_time(Duration::from_secs(10))
        .bench_function("100", bench_compute_hash_sorted::<100, Keccak256>())
        .bench_function("500", bench_compute_hash_sorted::<500, Keccak256>())
        .bench_function("1k", bench_compute_hash_sorted::<1000, Keccak256>())
        .bench_function("2k", bench_compute_hash_sorted::<2000, Keccak256>())
        .bench_function("5k", bench_compute_hash_sorted::<5000, Keccak256>())
        .bench_function("10k", bench_compute_hash_sorted::<10000, Keccak256>());

    c.benchmark_group("get() from a tree made with random values")
        .bench_function("1k", bench_get::<1_000>())
        .bench_function("10k", bench_get::<10_000>())
        .bench_function("100k", bench_get::<100_000>())
        .bench_function("1M", bench_get::<1_000_000>())
        .sample_size(10)
        .bench_function("10M", bench_get::<10_000_000>())
        .bench_function("100M", bench_get::<100_000_000>());

    c.benchmark_group("insert() from a tree made with random values")
        .bench_function("1k", bench_insert::<1_000>())
        .bench_function("10k", bench_insert::<10_000>())
        .bench_function("100k", bench_insert::<100_000>())
        .bench_function("1M", bench_insert::<1_000_000>())
        .sample_size(10)
        .bench_function("10M", bench_insert::<10_000_000>())
        .bench_function("100M", bench_insert::<100_000_000>());
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
