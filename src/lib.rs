//! # Patricia Merkle Tree

use self::{nibble::Nibble, node::Node};
use crate::nodes::LeafNode;
use digest::{Digest, Output};
use nibble::NibbleSlice;
use node::InsertAction;
use slab::Slab;
use std::mem::{replace, size_of};

pub mod nibble;
mod node;
mod nodes;
mod util;

type NodesStorage<P, V, H> = Slab<Node<P, V, H>>;
type ValuesStorage<P, V> = Slab<(P, V)>;

/// Patricia Merkle Tree implementation.
#[derive(Clone, Debug, Default)]
pub struct PatriciaMerkleTree<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    /// Reference to the root node.
    root_ref: usize,

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
            root_ref: 0,
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
        self.nodes.get(self.root_ref).and_then(|root_node| {
            root_node.get(&self.nodes, &self.values, NibbleSlice::new(path.as_ref()))
        })
    }

    /// Insert a value into the tree.
    pub fn insert(&mut self, path: P, value: V) -> Option<V> {
        match self.nodes.try_remove(self.root_ref) {
            Some(root_node) => {
                // If the tree is not empty, call the root node's insertion logic.
                let (root_node, insert_action) = root_node.insert(
                    &mut self.nodes,
                    &mut self.values,
                    NibbleSlice::new(path.as_ref()),
                );
                self.root_ref = self.nodes.insert(root_node);

                match insert_action.quantize_self(self.root_ref) {
                    InsertAction::Insert(node_ref) => {
                        let value_ref = self.values.insert((path, value));
                        match self
                            .nodes
                            .get_mut(node_ref)
                            .expect("inconsistent internal tree structure")
                        {
                            Node::Leaf(leaf_node) => leaf_node.update_value_ref(value_ref),
                            Node::Branch(branch_node) => {
                                branch_node.update_value_ref(Some(value_ref))
                            }
                            _ => panic!("inconsistent internal tree structure"),
                        };

                        None
                    }
                    InsertAction::Replace(value_ref) => {
                        let (_, old_value) = self
                            .values
                            .get_mut(value_ref)
                            .expect("inconsistent internal tree structure");

                        Some(replace(old_value, value))
                    }
                    _ => None,
                }
            }
            None => {
                // If the tree is empty, just add a leaf.
                let value_ref = self.values.insert((path, value));
                self.root_ref = self.nodes.insert(LeafNode::new(value_ref).into());

                None
            }
        }
    }

    // /// Remove a value from the tree.
    // pub fn remove(&mut self, path: &P) -> Option<V> {
    //     match self.nodes.try_remove(self.root_ref) {
    //         Some(root_node) => {
    //             // If the tree is not empty, call the root node's removal logic.
    //             let path_iter = Offseted::new(path.encoded_iter());
    //             let (root_node, old_value) =
    //                 root_node.remove(&mut self.nodes, &mut self.values, path_iter);
    //             if let Some(root_node) = root_node {
    //                 self.root_ref = self.nodes.insert(root_node);
    //             }

    //             old_value
    //         }
    //         None => None,
    //     }
    // }

    // TODO: Iterators.

    // /// Return the root hash of the tree (or recompute if needed).
    // pub fn compute_hash(&mut self) -> Option<Output<H>> {
    //     self.nodes.try_remove(self.root_ref).map(|mut root_node| {
    //         // TODO: Test what happens when the root node's hash encoding is hashed (len == 32).
    //         //   Double hash? Or forward the first one?
    //         let mut hasher = DigestBuf::<H>::new();
    //         hasher
    //             .write_all(root_node.compute_hash(&mut self.nodes, &self.values, 0))
    //             .unwrap();
    //         let output = hasher.finalize();

    //         self.root_ref = self.nodes.insert(root_node);
    //         output
    //     })
    // }

    /// Calculate approximated memory usage (both used and allocated).
    pub fn memory_usage(&self) -> (usize, usize) {
        let mem_consumed = size_of::<Node<P, V, H>>() * self.nodes.len()
            + size_of::<(P, Output<H>, V)>() * self.values.len();
        let mem_reserved = size_of::<Node<P, V, H>>() * self.nodes.capacity()
            + size_of::<(P, Output<H>, V)>() * self.values.capacity();

        (mem_consumed, mem_reserved)
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use proptest::collection::{hash_set, vec};
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
        fn proptest_get_inserted_multiple(paths in vec(vec(any::<u8>(), 1..5), 1..5), values in vec(vec(any::<u8>(), 1..5), 1..5)) {
            let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();

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
}
