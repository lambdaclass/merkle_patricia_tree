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
	echo "Benchmarking go-ethereum implementation:"
	cd ./external-benches/geth/; GOMAXPROCS=1 go test -bench=.
	echo "Benchmarking Paprika implementation (CSharp)"
	cd ./external-benches/paprika-bench/; dotnet run -c Release

ext-bench-prepare:
	cd ./external-benches/paprika-bench/
	dotnet nuget add source -n merkle_patricia_tree $(pwd)/nuget-feed

storage-bench:
	hyperfine --prepare 'cargo b --release --all-targets' -w 2 -L nodes 100,1000,10000,100000 'cargo r --release --example storage-sled {nodes}'
	hyperfine --prepare 'cargo b --release --all-targets' -w 2 -L nodes 100,1000,10000,100000 'cargo r --release --example storage-mdbx {nodes}'

profile:
	./profile.sh

clean-profile:
	rm -f dhat.out.* profile*.svg

coverage:
	cargo tarpaulin
