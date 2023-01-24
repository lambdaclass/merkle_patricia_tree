.PHONY: deps build check clippy test bench ext-bench coverage

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

ext-bench:
	cd ./external-benches/geth/; GOMAXPROCS=1 go test -bench=.

coverage:
	cargo tarpaulin
