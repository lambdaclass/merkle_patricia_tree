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
	 cargo build --examples --profile=release-with-debug && \
	 	valgrind --tool=dhat --dhat-out-file=dhat.out.n-100 ./target/release-with-debug/examples/calculate-root 100 && \
		valgrind --tool=dhat --dhat-out-file=dhat.out.n-1000 ./target/release-with-debug/examples/calculate-root 1000 && \
		valgrind --tool=dhat --dhat-out-file=dhat.out.n-10000 ./target/release-with-debug/examples/calculate-root 10000 && \
		valgrind --tool=dhat --dhat-out-file=dhat.out.n-100000 ./target/release-with-debug/examples/calculate-root 100000 && \
		valgrind --tool=dhat --dhat-out-file=dhat.out.n-500000 ./target/release-with-debug/examples/calculate-root 500000

coverage:
	cargo tarpaulin
