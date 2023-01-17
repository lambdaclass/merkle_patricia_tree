//! # Patricia Merkle Tree

#![deny(warnings)]

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
    borrow::Cow,
    mem::{replace, size_of},
};

mod dump;
mod hashing;
mod nibble;
mod node;
mod nodes;
mod storage;

/// Patricia Merkle Tree implementation.
#[derive(Clone, Debug, Default)]
pub struct PatriciaMerkleTree<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    /// Reference to the root node.
    root_ref: NodeRef,

    /// Contains all the nodes.
    nodes: NodesStorage<P, V, H>,
    /// Stores the actual nodes' hashed paths and values.
    values: ValuesStorage<P, V>,
}

impl<P, V, H> PatriciaMerkleTree<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    /// Create an empty tree.
    pub fn new() -> Self {
        Self {
            root_ref: NodeRef::default(),
            nodes: Slab::new(),
            values: Slab::new(),
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
        self.nodes.get(*self.root_ref).and_then(|root_node| {
            root_node.get(&self.nodes, &self.values, NibbleSlice::new(path.as_ref()))
        })
    }

    /// Insert a value into the tree.
    pub fn insert(&mut self, path: P, value: V) -> Option<V> {
        match self.nodes.try_remove(*self.root_ref) {
            Some(root_node) => {
                // If the tree is not empty, call the root node's insertion logic.
                let (root_node, insert_action) = root_node.insert(
                    &mut self.nodes,
                    &mut self.values,
                    NibbleSlice::new(path.as_ref()),
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
            }
            None => {
                // If the tree is empty, just add a leaf.
                let value_ref = ValueRef::new(self.values.insert((path, value)));
                self.root_ref = NodeRef::new(self.nodes.insert(LeafNode::new(value_ref).into()));

                None
            }
        }
    }

    /// Return the root hash of the tree (or recompute if needed).
    pub fn compute_hash(&mut self) -> Option<Cow<Output<H>>> {
        self.root_ref.is_valid().then(|| {
            let root_node = self
                .nodes
                .get_mut(*self.root_ref)
                .expect("inconsistent internal tree structure");

            match root_node.compute_hash(&mut self.nodes, &self.values, 0) {
                NodeHashRef::Inline(_) => todo!(),
                NodeHashRef::Hashed(x) => Cow::Borrowed(x),
            }
        })
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
    use crate::*;
    use proptest::collection::{btree_set, vec};
    use proptest::prelude::*;
    use sha3::Keccak256;

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
            assert!(item.is_some());
            let item = item.unwrap();
            assert_eq!(item, &value);
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
                assert!(item.is_some());
                assert_eq!(item.unwrap(), value);
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
    fn test() {
        let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();

        tree.insert(vec![48], vec![0]);
        tree.insert(vec![49], vec![1]);

        assert_eq!(tree.get(&vec![48]), Some(&vec![0]));
        assert_eq!(tree.get(&vec![49]), Some(&vec![1]));
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
}
