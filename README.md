# Patricia Merkle Tree implementation in Rust
[![Rust](https://github.com/lambdaclass/merkle_patricia_tree/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/lambdaclass/merkle_patricia_tree/actions/workflows/rust.yml)
[![codecov](https://codecov.io/gh/lambdaclass/merkle_patricia_tree/branch/main/graph/badge.svg?token=4EJ3CBOB0E)](https://codecov.io/gh/lambdaclass/merkle_patricia_tree)

This crate contains an implementation of a Patricia Tree.

Its structure is implemented to match Ethereum's [Patricia Merkle Trees](https://ethereum.org/en/developers/docs/data-structures-and-encoding/patricia-merkle-trie/).

> TODO: Storage implementation example.

## Building

```
make build
```

## Testing
```
make test
```

## Benchmarking

```
make bench
```

Benchmarks are provided for the following use cases:

  - Retrieval of non-existant nodes.
  - Retrieval of existing nodes.
  - Insertion of new nodes.
  - Overwriting nodes.
  - Removal of nodes.
  - Removal of non-existing nodes (no-op).

Every use case is tested with different tree sizes, ranging from 1k to 1M.

On a Apple M1 Pro (16gb RAM, macOS 13.1 22C65 arm64) using `Keccak256` as the hash function:
```
PatriciaMerkleTree::<MyNodePath, Keccak256>::get()/1k
                        time:   [40.057 ns 40.098 ns 40.142 ns]
PatriciaMerkleTree::<MyNodePath, Keccak256>::get()/10k
                        time:   [48.751 ns 48.760 ns 48.769 ns]
PatriciaMerkleTree::<MyNodePath, Keccak256>::get()/100k
                        time:   [203.78 ns 223.41 ns 248.22 ns]
PatriciaMerkleTree::<MyNodePath, Keccak256>::get()/1M
                        time:   [361.24 ns 395.14 ns 438.46 ns]
PatriciaMerkleTree::<MyNodePath, Keccak256>::insert()/1k
                        time:   [229.73 ns 229.81 ns 229.92 ns]
PatriciaMerkleTree::<MyNodePath, Keccak256>::insert()/10k
                        time:   [1.3474 µs 1.4288 µs 1.5149 µs]
PatriciaMerkleTree::<MyNodePath, Keccak256>::insert()/100k
                        time:   [3.7791 µs 3.8136 µs 3.8600 µs]
PatriciaMerkleTree::<MyNodePath, Keccak256>::insert()/1M
                        time:   [2.2547 µs 2.4548 µs 2.7034 µs]
```

Where `MyNode` is a path wrapper around a `Vec<u8>`

```
struct MyNodePath(Vec<u8>);
```


## What is a patricia merke tree

PATRICIA is an acronym which means:

Practical Algorithm to Retrieve Information Coded in Alphanumeric

> A compact representation of a trie in which any node that is an only child is merged with its parent. 

> Patricia tries are seen as radix trees with radix equals 2, which means that each bit of the key is compared individually and each node is a two-way (i.e., left versus right) branch

In essence, a patricia tree stores a value for the given key (also named **path**).

The key or path is converted to bytes, and then each nibble of each byte is used to traverse the tree.

It is composed of 3 different types of nodes:

### The branch node

It contains a 17 element array:
- The 16 first elements cover every representable value of a nibble (2^4 = 16)
- The value in case the path is fully traversed.

### The leaf node

It contains 2 elements:
- The encoded path.
- The value.

### The extension node

It contains 2 elements:
- The encoded path.
- A key or path for the next lookup.

This node allows the tree to be more compact, imagine we have a path that ultimately can only go 1 way, because it has no diverging paths,
adding X nodes instead of 1 representing that would be a waste of space, this fixes that.

For example, imagine we have the keys "abcdx" and "abcdy", instead of adding 10 nodes (1 for each nibble in each character), we create a single node representing the path "abcd", thus compressing the tree.


### Solving the ambiguity

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

## Terms Used
- **[nibble](https://en.wikipedia.org/wiki/Nibble)**: 4bits, half a byte, a single hex digit.

## Useful links

- [Patricia tree on NIST](https://xlinux.nist.gov/dads/HTML/patriciatree.html)
- [Paper by Donald R. Morrison](https://dl.acm.org/doi/10.1145/321479.321481)
