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
	 	rm -f data.dat && \
	 	echo -en "100 " >> data.dat && valgrind --tool=dhat --dhat-out-file=dhat.out.n-100 ./target/release-with-debug/examples/calculate-root 100 2>&1 | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$$1' | tr -d ',' | tr '\n' ' ' >> data.dat && \
		echo -en "\n1000 " >> data.dat && valgrind --tool=dhat --dhat-out-file=dhat.out.n-1000 ./target/release-with-debug/examples/calculate-root 1000 2>&1 | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$$1' | tr -d ',' | tr '\n' ' ' >> data.dat && \
		echo -en "\n10000 " >> data.dat && valgrind --tool=dhat --dhat-out-file=dhat.out.n-10000 ./target/release-with-debug/examples/calculate-root 10000 2>&1 | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$$1' | tr -d ',' | tr '\n' ' ' >> data.dat && \
		echo -en "\n50000 " >> data.dat && valgrind --tool=dhat --dhat-out-file=dhat.out.n-50000 ./target/release-with-debug/examples/calculate-root 50000 2>&1 | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$$1' | tr -d ',' | tr '\n' ' ' >> data.dat && \
		echo -en "\n100000 " >> data.dat && valgrind --tool=dhat --dhat-out-file=dhat.out.n-100000 ./target/release-with-debug/examples/calculate-root 100000 2>&1 | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$$1' | tr -d ',' | tr '\n' ' ' >> data.dat && \
		echo -en "\n250000 " >> data.dat && valgrind --tool=dhat --dhat-out-file=dhat.out.n-250000 ./target/release-with-debug/examples/calculate-root 250000 2>&1 | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$$1' | tr -d ',' | tr '\n' ' ' >> data.dat && \
		echo -en "\n500000 " >> data.dat && valgrind --tool=dhat --dhat-out-file=dhat.out.n-500000 ./target/release-with-debug/examples/calculate-root 500000 2>&1 | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$$1' | tr -d ',' | tr '\n' ' ' >> data.dat && \
		echo -en "\n1000000 " >> data.dat && valgrind --tool=dhat --dhat-out-file=dhat.out.n-1000000 ./target/release-with-debug/examples/calculate-root 1000000 2>&1 | rg -A 4 'Total:' | sed -E 's/==\w+== //g' | rg -o ':\s+([0-9,]+)' -r '$$1' | tr -d ',' | tr '\n' ' ' >> data.dat && \
		gnuplot plot-profile.plt

clean-profile:
	rm -f data.dat dhat.out.* profile.svg

coverage:
	cargo tarpaulin
