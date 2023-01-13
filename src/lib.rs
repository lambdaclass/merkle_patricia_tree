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

// #[cfg(test)]
// mod test {
//     use super::*;
//     use crate::nibble::NibbleIterator;
//     use sha3::Keccak256;
//     use std::{io, str::Bytes};

//     #[derive(Clone, Debug, Eq, PartialEq)]
//     struct MyNodePath(String);

//     impl TreePath for MyNodePath {
//         type Iterator<'a> = NibbleIterator<Bytes<'a>>;

//         fn encode(&self, mut target: impl io::Write) -> io::Result<()> {
//             target.write_all(self.0.as_bytes())
//         }

//         fn encoded_iter(&self) -> Self::Iterator<'_> {
//             NibbleIterator::new(self.0.bytes())
//         }
//     }

//     // Temporary test for bug.
//     #[test]
//     fn test() {
//         let mut pmt = PatriciaMerkleTree::<MyNodePath, [u8; 0], Keccak256>::new();

//         pmt.insert(MyNodePath("ab".to_string()), []);
//         pmt.insert(MyNodePath("ac".to_string()), []);
//         pmt.insert(MyNodePath("a".to_string()), []);
//     }
// }

#[cfg(test)]
mod test {
    use super::*;
    use sha3::Keccak256;

    #[test]
    fn test() {
        let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();

        tree.insert(vec![48], vec![0]);
        tree.insert(vec![49], vec![1]);

        assert_eq!(tree.get(&vec![48]), Some(&vec![0]));
        assert_eq!(tree.get(&vec![49]), Some(&vec![1]));
    }
}
