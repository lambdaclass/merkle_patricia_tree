<div align="center">

### 🦀🌲 Patricia Merkle Tree in Rust 🌲🦀

Implementation of Ethereum's Patricia Merkle tree in Rust

[Report Bug](https://github.com/lambdaclass/merkle_patricia_tree/issues/new?labels=bug&title=bug%3A+) · [Request Feature](https://github.com/lambdaclass/merkle_patricia_tree/issues/new?labels=enhancement&title=feat%3A+)

[![Rust](https://github.com/lambdaclass/merkle_patricia_tree/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/lambdaclass/merkle_patricia_tree/actions/workflows/rust.yml)
[![codecov](https://img.shields.io/codecov/c/github/lambdaclass/merkle_patricia_tree)](https://codecov.io/gh/lambdaclass/merkle_patricia_tree)
[![license](https://img.shields.io/github/license/lambdaclass/merkle_patricia_tree)](/LICENSE)
[![pr-welcome]](#-contributing)

[pr-welcome]: https://img.shields.io/static/v1?color=orange&label=PRs&style=flat&message=welcome

</div>

## Table of Contents

- [Disclaimer](#%EF%B8%8F-disclaimer)
- [About](#-about)
- [Usage](#-usage)
  * [Testing](#testing)
- [Benchmarking](#-benchmarking)
- [Contributing](#-contributing)
- [Documentation](#-documentation)
  * [What is a patricia merke tree](#what-is-a-patricia-merke-tree)
  * [Terms Used](#terms-used)
  * [Useful links](#useful-links)
- [License](#%EF%B8%8F-license)

## ⚠️ Disclaimer

🚧 This project is a work-in-progress and is not ready for production yet. Use at your own risk. 🚧

## 📖 About

This crate contains an implementation of a Patricia Tree.

Its structure is implemented to match Ethereum's [Patricia Merkle Trees](https://ethereum.org/en/developers/docs/data-structures-and-encoding/patricia-merkle-trie/).

## 🚀 Usage

Here's an example to calculate the hash after inserting a few items.

```rust
use merkle_patricia_tree::PatriciaMerkleTree;
use sha3::Keccak256;

let mut tree = PatriciaMerkleTree::<&[u8], &[u8], Keccak256>::new();
tree.insert(b"doe", b"reindeer");
tree.insert(b"dog", b"puppy");
tree.insert(b"dogglesworth", b"cat");

let hash = tree.compute_hash().unwrap();
println!("{hash:02x}");
```

### Testing

Run the following command:

```
make test
```

## 📊 Benchmarking

```
make bench
```

To run external benches:


Run the one-time setup
```
make ext-bench-prepare
```

```
make ext-bench
```

Benchmarks are provided for the following use cases:

  - Retrieval of non-existant nodes.
  - Retrieval of existing nodes.
  - Insertion of new nodes.
  - Overwriting nodes.
  - Removal of nodes.
  - Removal of non-existing nodes (no-op).
  - Calculate the root Keccak256 hash.

Every use case is tested with different tree sizes, ranging from 1k to 1M.

On a AMD Ryzen 9 5950x 3.4 Ghz with 128 Gb RAM using `Keccak256` as the hash function:

| Bench | 1k | 10k | 100k | 1m | 10m | 100m |
|----------|------|-----------|-------------|----|---|---|
| lambda's get() | `38.287 ns` | `58.692 ns` | `118.90 ns` | `266.56 ns` | `365.52 ns` | `528.04 ns` |
| geth get() | `110.7 ns` | `139.6 ns` | `247.6 ns` | `484.5 ns` | `1286 ns` | `timeout` |
| paprika get() | `48.14 ns` | `57.97 ns` | `77.95 ns` | `192.25 ns` | `244.59 ns` | `timeout (memory)` |
| lambda's insert() | `327.44 ns` | `407.50 ns` | `778.76 ns` | `1.6858 µs` | `4.6706 µs` | `4.9003 µs` |
| geth insert() | `536.3 ns` | `820.3 ns` | `1.624 µs` | `2.649 µs` | `6.522 µs` | `timeout` |
| paprika insert() | `2.251 ns` | `1.964 ns` | `3.650 µs` | `5.391 µs` | `5.270 us` | `timeout (memory)` |

| Bench | 100 | 500 | 1k | 2k | 5k | 10k |
|----------|------|-----------|-------------|----|---|---|
| lambda's root Keccak256 | `113.63 µs` | `557.49 µs` | `1.1775 ms` | `2.3716 ms` | `5.8113 ms` | `11.737 ms` |
| geth root Keccak256 | `102.358 µs` | `504.081 µs` | `989.531 µs` | `1.936 ms` | `5.59 ms` | `11.458 ms` |

Gets | Inserts
:----:|:---:
<img src="plots/bench-gets.svg?raw=true" width="100%"> | <img src="plots/bench-inserts.svg?raw=true" width="100%">

Requires hyperfine:

```bash
make storage-bench
```

| Storage Bench | 100 | 1k | 10k | 1m |
|----------|------|-----------|-------------|--------|
| sled insert + hash | `210.4 ms` | `204.6 ms` | `245.1 ms` | `861.3 ms` |
| libmdx insert + hash | `195.5 ms` | `262.3 ms` | `1.002 s` | `7.93 s` |

## Profiling

Dependencies: valgrind, gnuplot, make

You can profile some example programs and generate plots using the following command:

```
make profile
```

Normal | From Sorted Iter
:----:|:---:
<img src="plots/profile.svg?raw=true" width="100%"> | <img src="plots/profile-sorted.svg?raw=true" width="100%">

<img src="plots/profile-both.svg?raw=true" width="100%">

## 🛠 Contributing

The open source community is a fantastic place for learning, inspiration, and creation, and this is all thanks to contributions from people like you. Your contributions are **greatly appreciated**.

If you have any suggestions for how to improve the project, please feel free to fork the repo and create a pull request, or [open an issue](https://github.com/lambdaclass/merkle_patricia_tree/issues/new?labels=enhancement&title=feat%3A+) with the tag 'enhancement'.

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## 📚 Documentation

### What is a Patricia Merkle Tree

PATRICIA is an acronym which means:

Practical Algorithm to Retrieve Information Coded in Alphanumeric

> A compact representation of a trie in which any node that is an only child is merged with its parent.

> Patricia tries are seen as radix trees with radix equals 2, which means that each bit of the key is compared individually and each node is a two-way (i.e., left versus right) branch

In essence, a patricia tree stores a value for the given path.

The path is encoded into bytes, and then each nibble of each byte is used to traverse the tree.

It is composed of 3 different types of nodes:

#### The branch node

It contains a 17 element array:
- The 16 first elements cover every representable value of a nibble (2^4 = 16)
- The value in case the path is fully traversed.

#### The leaf node

It contains 2 elements:
- The encoded path.
- The value.

#### The extension node

It contains 2 elements:
- The prefix as a segment of the path.
- A reference to the child node (which must be a branch).

This node allows the tree to be more compact, imagine we have a path that ultimately can only go 1 way, because it has no diverging paths,
adding X nodes instead of 1 representing that would be a waste of space, this fixes that.

For example, imagine we have the paths "abcdx" and "abcdy", instead of adding 10 nodes (1 for each nibble in each character), we create a single node representing the path "abcd", thus compressing the tree.


#### Solving the ambiguity

Since traversing a path is done through it's nibbles, when doing so, the remaining partial path may have an odd number of nibbles left, this
introduces an ambiguity that comes from storing a nibble as a byte:

Imagine we have the following remaining nibbles:

- `1`
- `01`

When representing both as a byte, they have the same value `1`.

Thats why a flag is introduced to differenciate between an odd or even remaining partial path:

| hex char | bits | node type | path length |
|----------|------|-----------|-------------|
| 0        | 0000 | extension | even        |
| 1        | 0001 | extension | odd         |
| 2        | 0010 | leaf      | even        |
| 3        | 0011 | leaf      | odd         |

```
[flag] + path
```

### Terms Used
- **[nibble](https://en.wikipedia.org/wiki/Nibble)**: 4bits, half a byte, a single hex digit.

### Useful links

- [Patricia tree on NIST](https://xlinux.nist.gov/dads/HTML/patriciatree.html)
- [Paper by Donald R. Morrison](https://dl.acm.org/doi/10.1145/321479.321481)

## ⚖️ License

This project is licensed under the Apache 2.0 license.

See [LICENSE](/LICENSE) for more information.
