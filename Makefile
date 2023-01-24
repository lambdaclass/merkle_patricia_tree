.PHONY: deps build check clippy test bench coverage

build:
	cargo build --release

deps:
	cargo install cargo-tarpaulin

check:
	cargo check --all-targets

clippy:
	cargo clippy --all-targets

test:
	cargo test

bench:
	cargo bench

profile:
	 cargo build --examples --profile=release-with-debug && valgrind --tool=dhat ./target/release-with-debug/examples/calculate-root

coverage:
	cargo tarpaulin
