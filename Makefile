.PHONY: deps build check clippy test bench coverage

build:
	cargo build --release

deps:
	cargo install cargo-tarpaulin

check:
	cargo check --all-targets

clippy:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test

bench:
	cargo bench

coverage:
	cargo tarpaulin
