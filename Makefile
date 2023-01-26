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

# External benches dependencies: go, dotnet-sdk
ext-bench:
	cd ./external-benches/geth/; GOMAXPROCS=1 go test -bench=.
	cd ./external-benches/paprika-bench/; dotnet run --configuration Release

ext-bench-prepare:
	cd ./external-benches/paprika-bench/
	dotnet nuget add source -n merkle_patricia_tree $(pwd)/nuget-feed

coverage:
	cargo tarpaulin
