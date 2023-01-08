//! # Patricia Merkle Tree
//!
//! **Features**
//!   - Variable-length keys.
//!   -

use self::node::Node;
pub use self::{
    nibble::{Nibble, NibbleIterator},
    path::TreePath,
};
use crate::{nodes::LeafNode, util::build_value};
use digest::{Digest, Output};
use node::InsertAction;
use slab::Slab;
use std::mem::{replace, size_of};
use util::Offseted;

mod nibble;
mod node;
mod nodes;
mod path;
mod util;

type NodesStorage<P, V, H> = Slab<Node<P, V, H>>;
type ValuesStorage<P, V, H> = Slab<(P, Output<H>, V)>;

/// Patricia Merkle Tree implementation.
///
/// The value `V` should contain the path, which should be retrievable using the `TreePath` trait.
/// This is made because:
///   - There isn't always a value.
///   - Sometimes the value itself may be the path.
///   - The path may have to be preprocessed (rlp encoding, for example).
/// By using a trait like `TreePath`, all this complexity can be easily implemented.
#[derive(Clone, Default)]
pub struct PatriciaMerkleTree<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    /// Reference to the root node.
    root_ref: usize,

    /// Contains all the nodes.
    nodes: NodesStorage<P, V, H>,
    /// Stores the actual nodes' hashed paths and values.
    values: ValuesStorage<P, V, H>,
}

impl<P, V, H> PatriciaMerkleTree<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    pub fn new() -> Self {
        Self {
            root_ref: 0,
            nodes: Slab::new(),
            values: Slab::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Retrieves a value from the tree given its path.
    pub fn get(&self, path: &P) -> Option<&V> {
        self.nodes.get(self.root_ref).and_then(|root_node| {
            let path_iter = Offseted::new(path.encoded_iter());
            root_node.get(&self.nodes, &self.values, path_iter)
        })
    }

    pub fn insert(&mut self, path: P, value: V) -> Option<V>
    where
        P: Clone,
    {
        match self.nodes.try_remove(0) {
            Some(root_node) => {
                // If the tree is not empty, call the root node's insertion logic.
                let path_iter = Offseted::new(path.encoded_iter());
                let (root_node, insert_action) =
                    root_node.insert(&mut self.nodes, &mut self.values, path_iter);
                self.root_ref = self.nodes.insert(root_node);

                match insert_action.quantize_self(self.root_ref) {
                    InsertAction::Insert(node_ref) => {
                        let value_ref = self.values.insert(build_value::<P, V, H>(path, value));
                        match self
                            .nodes
                            .get_mut(node_ref)
                            .expect("inconsistent internal tree structure")
                        {
                            Node::Leaf(leaf_node)
                            | Node::LeafBranch(_, leaf_node)
                            | Node::LeafExtension(_, leaf_node) => {
                                leaf_node.update_value_ref(value_ref);
                            }
                            _ => panic!("inconsistent_internal_tree_structure"),
                        };

                        None
                    }
                    InsertAction::Replace(value_ref) => {
                        let (_, _, old_value) = self
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
                let value_ref = self.values.insert(build_value::<P, V, H>(path, value));
                let node_ref = self.nodes.insert(LeafNode::new(value_ref).into());
                assert_eq!(node_ref, 0, "inconsistent internal tree structure");

                None
            }
        }
    }

    // pub fn remove(&mut self, _path: &V::Path) -> Option<V> {
    //     todo!()
    // }

    // // TODO: Iterators.

    // pub fn compute_hash(&self) -> Output<H> {
    //     todo!()
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
