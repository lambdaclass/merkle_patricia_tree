//! # Patricia Merkle Tree

#![deny(warnings)]

pub use self::codec::Encode;
use self::{
    nibble::NibbleSlice,
    node::{InsertAction, Node},
    nodes::LeafNode,
    storage::{NodeRef, NodesStorage, ValueRef, ValuesStorage},
};
use digest::{Digest, Output};
use hashing::NodeHashRef;
use slab::Slab;
use std::{
    fmt::Debug,
    mem::{replace, size_of},
};

mod codec;
#[cfg(feature = "tree-dump")]
pub mod dump;
mod hashing;
mod nibble;
mod node;
mod nodes;
mod storage;
mod util;

/// Patricia Merkle Tree implementation.
#[derive(Clone, Debug, Default)]
pub struct PatriciaMerkleTree<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    /// Reference to the root node.
    root_ref: NodeRef,

    /// Contains all the nodes.
    nodes: NodesStorage<P, V, H>,
    /// Stores the actual nodes' hashed paths and values.
    values: ValuesStorage<P, V>,

    hash: (bool, Output<H>),
}

impl<P, V, H> PatriciaMerkleTree<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    /// Create an empty tree.
    pub fn new() -> Self {
        Self {
            root_ref: NodeRef::default(),
            nodes: Slab::new(),
            values: Slab::new(),
            hash: (false, Default::default()),
        }
    }

    /// Return whether the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Return the number of values in the tree.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Retrieve a value from the tree given its path.
    pub fn get(&self, path: &P) -> Option<&V> {
        if !self.root_ref.is_valid() {
            return None;
        }

        let root_node = self
            .nodes
            .get(*self.root_ref)
            .expect("inconsistent internal tree structure");

        let encoded_path = path.encode();
        root_node.get(
            &self.nodes,
            &self.values,
            NibbleSlice::new(encoded_path.as_ref()),
        )
    }

    /// Insert a value into the tree.
    pub fn insert(&mut self, path: P, value: V) -> Option<V> {
        // Mark hash as dirty.
        self.hash.0 = false;

        if let Some(root_node) = self.nodes.try_remove(*self.root_ref) {
            // If the tree is not empty, call the root node's insertion logic.
            let encoded_path = path.encode();
            let (root_node, insert_action) = root_node.insert(
                &mut self.nodes,
                &mut self.values,
                NibbleSlice::new(encoded_path.as_ref()),
            );
            self.root_ref = NodeRef::new(self.nodes.insert(root_node));

            match insert_action.quantize_self(self.root_ref) {
                InsertAction::Insert(node_ref) => {
                    let value_ref = ValueRef::new(self.values.insert((path, value)));
                    match self
                        .nodes
                        .get_mut(*node_ref)
                        .expect("inconsistent internal tree structure")
                    {
                        Node::Leaf(leaf_node) => leaf_node.update_value_ref(value_ref),
                        Node::Branch(branch_node) => branch_node.update_value_ref(value_ref),
                        _ => panic!("inconsistent internal tree structure"),
                    };

                    None
                }
                InsertAction::Replace(value_ref) => {
                    let (_, old_value) = self
                        .values
                        .get_mut(*value_ref)
                        .expect("inconsistent internal tree structure");

                    Some(replace(old_value, value))
                }
                _ => unreachable!(),
            }
        } else {
            // If the tree is empty, just add a leaf.
            let value_ref = ValueRef::new(self.values.insert((path, value)));
            self.root_ref = NodeRef::new(self.nodes.insert(LeafNode::new(value_ref).into()));

            None
        }
    }

    /// Remove a value from the tree.
    pub fn remove(&mut self, path: P) -> Option<V> {
        if !self.root_ref.is_valid() {
            return None;
        }

        let root_node = self
            .nodes
            .try_remove(*self.root_ref)
            .expect("inconsistent internal tree structure");
        let (root_node, old_value) = root_node.remove(
            &mut self.nodes,
            &mut self.values,
            NibbleSlice::new(path.encode().as_ref()),
        );
        self.root_ref = match root_node {
            Some(root_node) => NodeRef::new(self.nodes.insert(root_node)),
            None => Default::default(),
        };

        old_value
    }

    /// Return the root hash of the tree (or recompute if needed).
    pub fn compute_hash(&mut self) -> &Output<H> {
        if !self.hash.0 {
            if self.root_ref.is_valid() {
                let root_node = self
                    .nodes
                    .get(*self.root_ref)
                    .expect("inconsistent internal tree structure");

                match root_node.compute_hash(&self.nodes, &self.values, 0) {
                    NodeHashRef::Inline(x) => {
                        H::new().chain_update(&*x).finalize_into(&mut self.hash.1)
                    }
                    NodeHashRef::Hashed(x) => self.hash.1.copy_from_slice(&x),
                }
            } else {
                H::new()
                    .chain_update([0x80])
                    .finalize_into(&mut self.hash.1);
            }
            self.hash.0 = true;
        }
        &self.hash.1
    }

    /// Generate a tree from a sorted items iterator.
    ///
    /// Panics if the iterator is not sorted.
    pub fn from_sorted_iter(iter: impl IntoIterator<Item = (P, V)>) -> Self {
        let mut tree = Self::new();
        for (path, value) in iter {
            tree.insert(path, value);
        }

        tree
    }

    /// Compute the root hash of a tree given a ascending sorted iterator to its items.
    ///
    /// Panics if the iterator is not sorted.
    pub fn compute_hash_from_sorted_iter<'a>(
        iter: impl IntoIterator<Item = &'a (P, V)>,
    ) -> Output<H>
    where
        P: 'a,
        V: 'a,
    {
        util::compute_hash_from_sorted_iter::<P, V, H>(iter)
    }

    /// Calculate approximated memory usage (both used and allocated).
    pub fn memory_usage(&self) -> (usize, usize) {
        let mem_consumed = size_of::<Node<P, V, H>>() * self.nodes.len()
            + size_of::<(P, Output<H>, V)>() * self.values.len();
        let mem_reserved = size_of::<Node<P, V, H>>() * self.nodes.capacity()
            + size_of::<(P, Output<H>, V)>() * self.values.capacity();

        (mem_consumed, mem_reserved)
    }

    /// Use after a `.clone()` to reserve the capacity the slabs would have if they hadn't been
    /// cloned.
    ///
    /// Note: Used by the benchmark to mimic real conditions.
    #[doc(hidden)]
    pub fn reserve_next_power_of_two(&mut self) {
        self.nodes
            .reserve(self.nodes.capacity().next_power_of_two());
        self.values
            .reserve(self.values.capacity().next_power_of_two());
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::*;
    use hex_literal::hex;
    use proptest::collection::{btree_set, vec};
    use proptest::prelude::*;
    use sha3::Keccak256;

    #[test]
    fn compute_hash() {
        let mut tree = PatriciaMerkleTree::<&[u8], &[u8], Keccak256>::new();

        tree.insert(b"first", b"value");
        tree.insert(b"second", b"value");

        assert_eq!(
            tree.compute_hash().as_slice(),
            hex!("f7537e7f4b313c426440b7fface6bff76f51b3eb0d127356efbe6f2b3c891501"),
        );
    }

    #[test]
    fn compute_hash_long() {
        let mut tree = PatriciaMerkleTree::<&[u8], &[u8], Keccak256>::new();

        tree.insert(b"first", b"value");
        tree.insert(b"second", b"value");
        tree.insert(b"third", b"value");
        tree.insert(b"fourth", b"value");

        assert_eq!(
            tree.compute_hash().as_slice(),
            hex!("e2ff76eca34a96b68e6871c74f2a5d9db58e59f82073276866fdd25e560cedea"),
        );
    }

    #[test]
    fn get_inserted() {
        let mut tree = PatriciaMerkleTree::<&[u8], &[u8], Keccak256>::new();

        tree.insert(b"first", b"value");
        tree.insert(b"second", b"value");

        let first = tree.get(&&b"first"[..]);
        assert!(first.is_some());
        let second = tree.get(&&b"second"[..]);
        assert!(second.is_some());
    }

    #[test]
    fn get_inserted_zero() {
        let mut tree = PatriciaMerkleTree::<&[u8], &[u8], Keccak256>::new();

        tree.insert(&[0x0], b"value");
        let first = tree.get(&&[0x0][..]);
        assert!(first.is_some());
    }

    proptest! {
        #[test]
        fn proptest_get_inserted(path in vec(any::<u8>(), 1..100), value in vec(any::<u8>(), 1..100)) {
            let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();

            tree.insert(path.clone(), value.clone());
            let item = tree.get(&path);
            prop_assert!(item.is_some());
            let item = item.unwrap();
            prop_assert_eq!(item, &value);
        }

        #[test]
        fn proptest_get_inserted_multiple(paths in btree_set(vec(any::<u8>(), 1..100), 1..100)) {
            let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();

            let paths: Vec<Vec<u8>> = paths.into_iter().collect();
            let values = paths.clone();

            for (path, value) in paths.iter().zip(values.iter()) {
                tree.insert(path.clone(), value.clone());
            }

            for (path, value) in paths.iter().zip(values.iter()) {
                let item = tree.get(path);
                prop_assert!(item.is_some());
                prop_assert_eq!(item.unwrap(), value);
            }
        }
    }

    #[test]
    // cc 27153906dcbc63f2c7af31f8d0f600cd44bddd133d806d251a8a4125fff8b082 # shrinks to paths = [[16], [16, 0]], values = [[0], [0]]
    fn proptest_regression_27153906dcbc63f2c7af31f8d0f600cd44bddd133d806d251a8a4125fff8b082() {
        let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();
        tree.insert(vec![16], vec![0]);
        tree.insert(vec![16, 0], vec![0]);

        let item = tree.get(&vec![16]);
        assert!(item.is_some());
        assert_eq!(item.unwrap(), &vec![0]);

        let item = tree.get(&vec![16, 0]);
        assert!(item.is_some());
        assert_eq!(item.unwrap(), &vec![0]);
    }

    #[test]
    // cc 1b641284519306a352e730a589e07098e76c8a433103b50b3d82422f8d552328 # shrinks to paths = {[1, 0], [0, 0]}
    fn proptest_regression_1b641284519306a352e730a589e07098e76c8a433103b50b3d82422f8d552328() {
        let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();
        tree.insert(vec![0, 0], vec![0, 0]);
        tree.insert(vec![1, 0], vec![1, 0]);

        let item = tree.get(&vec![1, 0]);
        assert!(item.is_some());
        assert_eq!(item.unwrap(), &vec![1, 0]);

        let item = tree.get(&vec![0, 0]);
        assert!(item.is_some());
        assert_eq!(item.unwrap(), &vec![0, 0]);
    }

    #[test]
    fn proptest_regression_247af0efadcb3a37ebb8f9e3258dc2096d295201a7c634a5470b2f17385417e1() {
        let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();

        tree.insert(vec![26, 192, 44, 251], vec![26, 192, 44, 251]);
        tree.insert(
            vec![195, 132, 220, 124, 112, 201, 70, 128, 235],
            vec![195, 132, 220, 124, 112, 201, 70, 128, 235],
        );
        tree.insert(vec![126, 138, 25, 245, 146], vec![126, 138, 25, 245, 146]);
        tree.insert(
            vec![129, 176, 66, 2, 150, 151, 180, 60, 124],
            vec![129, 176, 66, 2, 150, 151, 180, 60, 124],
        );
        tree.insert(vec![138, 101, 157], vec![138, 101, 157]);

        let item = tree.get(&vec![26, 192, 44, 251]);
        assert!(item.is_some());
        assert_eq!(item.unwrap(), &vec![26, 192, 44, 251]);

        let item = tree.get(&vec![195, 132, 220, 124, 112, 201, 70, 128, 235]);
        assert!(item.is_some());
        assert_eq!(
            item.unwrap(),
            &vec![195, 132, 220, 124, 112, 201, 70, 128, 235]
        );

        let item = tree.get(&vec![126, 138, 25, 245, 146]);
        assert!(item.is_some());
        assert_eq!(item.unwrap(), &vec![126, 138, 25, 245, 146]);

        let item = tree.get(&vec![129, 176, 66, 2, 150, 151, 180, 60, 124]);
        assert!(item.is_some());
        assert_eq!(
            item.unwrap(),
            &vec![129, 176, 66, 2, 150, 151, 180, 60, 124]
        );

        let item = tree.get(&vec![138, 101, 157]);
        assert!(item.is_some());
        assert_eq!(item.unwrap(), &vec![138, 101, 157]);
    }

    fn insert_vecs(
        tree: &mut PatriciaMerkleTree<Vec<u8>, Vec<u8>, Keccak256>,
        vecs: &Vec<Vec<u8>>,
    ) {
        for x in vecs {
            tree.insert(x.clone(), x.clone());
        }
    }

    fn check_vecs(tree: &mut PatriciaMerkleTree<Vec<u8>, Vec<u8>, Keccak256>, vecs: &Vec<Vec<u8>>) {
        for x in vecs {
            let item = tree.get(x);
            assert!(item.is_some());
            assert_eq!(item.unwrap(), x);
        }
    }

    #[test]
    fn proptest_regression_3a00543dc8638a854e0e97892c72c1afb55362b9a16f7f32f0b88e6c87c77a4d() {
        let vecs = vec![
            vec![52, 53, 143, 52, 206, 112],
            vec![14, 183, 34, 39, 113],
            vec![55, 5],
            vec![134, 123, 19],
            vec![0, 59, 240, 89, 83, 167],
            vec![22, 41],
            vec![13, 166, 159, 101, 90, 234, 91],
            vec![31, 180, 161, 122, 115, 51, 37, 61, 101],
            vec![208, 192, 4, 12, 163, 254, 129, 206, 109],
        ];

        let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();

        insert_vecs(&mut tree, &vecs);
        check_vecs(&mut tree, &vecs);
    }

    #[test]
    fn proptest_regression_72044483941df7c265fa4a9635fd6c235f7790f35d878277fea7955387e59fea() {
        let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();

        tree.insert(vec![0x00], vec![0x00]);
        tree.insert(vec![0xC8], vec![0xC8]);
        tree.insert(vec![0xC8, 0x00], vec![0xC8, 0x00]);

        assert_eq!(tree.get(&vec![0x00]), Some(&vec![0x00]));
        assert_eq!(tree.get(&vec![0xC8]), Some(&vec![0xC8]));
        assert_eq!(tree.get(&vec![0xC8, 0x00]), Some(&vec![0xC8, 0x00]));
    }

    #[test]
    fn proptest_regression_4f3f0c44fdba16d943c33475dc4fa4431123ca274d17e3529dc7aa778de5655b() {
        let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();

        tree.insert(vec![0x00], vec![0x00]);
        tree.insert(vec![0x01], vec![0x01]);
        tree.insert(vec![0x10], vec![0x10]);
        tree.insert(vec![0x19], vec![0x19]);
        tree.insert(vec![0x19, 0x00], vec![0x19, 0x00]);
        tree.insert(vec![0x1A], vec![0x1A]);

        assert_eq!(tree.get(&vec![0x00]), Some(&vec![0x00]));
        assert_eq!(tree.get(&vec![0x01]), Some(&vec![0x01]));
        assert_eq!(tree.get(&vec![0x10]), Some(&vec![0x10]));
        assert_eq!(tree.get(&vec![0x19]), Some(&vec![0x19]));
        assert_eq!(tree.get(&vec![0x19, 0x00]), Some(&vec![0x19, 0x00]));
        assert_eq!(tree.get(&vec![0x1A]), Some(&vec![0x1A]));
    }

    #[test]
    fn compute_hashes() {
        expect_hash(vec![
            (b"doe".to_vec(), b"reindeer".to_vec()),
            (b"dog".to_vec(), b"puppy".to_vec()),
            (b"dogglesworth".to_vec(), b"cat".to_vec()),
        ])
        .unwrap();
    }

    proptest! {
        #[test]
        fn proptest_compare_hashes_simple(path in vec(any::<u8>(), 1..32), value in vec(any::<u8>(), 1..100)) {
            expect_hash(vec![(path, value)])?;
        }

        #[test]
        fn proptest_compare_hashes_multiple(data in btree_set((vec(any::<u8>(), 1..32), vec(any::<u8>(), 1..100)), 1..100)) {
            expect_hash(data.into_iter().collect())?;
        }
    }

    fn expect_hash(data: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), TestCaseError> {
        prop_assert_eq!(
            compute_hash_cita_trie(data.clone()),
            compute_hash_ours(data)
        );
        Ok(())
    }

    fn compute_hash_ours(data: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
        let mut tree = PatriciaMerkleTree::<_, _, Keccak256>::new();

        for (path, val) in data {
            tree.insert(path, val);
        }

        tree.compute_hash().as_slice().to_vec()
    }

    fn compute_hash_cita_trie(data: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
        use cita_trie::MemoryDB;
        use cita_trie::{PatriciaTrie, Trie};
        use hasher::HasherKeccak;

        let memdb = Arc::new(MemoryDB::new(true));
        let hasher = Arc::new(HasherKeccak::new());

        let mut trie = PatriciaTrie::new(Arc::clone(&memdb), Arc::clone(&hasher));

        for (path, value) in data {
            trie.insert(path.to_vec(), value.to_vec()).unwrap();
        }

        trie.root().unwrap()
    }
}
