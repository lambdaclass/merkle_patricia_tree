//! # Patricia Merkle Tree
//!
//! **Features**
//!   - Variable-length keys.
//!   -

use self::node::Node;
pub use self::path::TreePath;
use crate::{nodes::LeafNode, util::build_value};
use digest::{Digest, Output};
use generic_array::ArrayLength;
use nibble::NibbleIterator;
use slab::Slab;
use std::{cell::RefCell, io::Cursor, ops::DerefMut};

mod nibble;
mod node;
mod nodes;
mod path;
mod util;

/// Patricia Merkle Tree implementation.
///
/// The value `V` should contain the path, which should be retrievable using the `TreePath` trait.
/// This is made because:
///   - There isn't always a value.
///   - Sometimes the value itself may be the path.
///   - The path may have to be preprocessed (rlp encoding, for example).
/// By using a trait like `TreePath`, all this complexity can be easily implemented.
#[derive(Default)]
pub struct PatriciaMerkleTree<V, H>
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
{
    /// Reference to the root node.
    root_ref: usize,

    /// Contains all the nodes.
    nodes: Slab<Node<V, H>>,
    /// Stores the actual nodes' hashed paths and values.
    values: Slab<(<H::OutputSize as ArrayLength<u8>>::ArrayType, V)>,

    /// Used (and reused) internally to avoid allocating memory every time a buffer is needed.
    buffer: RefCell<Vec<u8>>,
}

impl<V, H> PatriciaMerkleTree<V, H>
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
{
    pub fn new() -> Self {
        Self {
            root_ref: 0,
            nodes: Slab::new(),
            values: Slab::new(),
            buffer: RefCell::new(Vec::new()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Retrieves a value from the tree given its path.
    pub fn get(&self, path: &V::Path) -> Option<&V> {
        self.nodes.get(self.root_ref).and_then(|root_node| {
            let mut buffer = self.buffer.borrow_mut();

            // Encode the path into the buffer.
            buffer.clear();
            V::encode_path(path, Cursor::new(buffer.deref_mut()));

            // Call the root node's getter logic.
            let path_iter = NibbleIterator::new(buffer.iter().copied());
            root_node.get(&self.nodes, &self.values, path, path_iter)
        })
    }

    pub fn insert(&mut self, value: V) -> Option<V> {
        // TODO: Maybe use a crate to get ownership (unsafely) from a ref?
        //   Maybe Cell::update? (is experimental, Cell should be on Slab).
        match self.nodes.try_remove(0) {
            Some(root_node) => {
                // If the tree is not empty, call the root node's insertion logic.
                let buffer = self.buffer.get_mut();
                let path = value.path();

                buffer.clear();
                V::encode_path(&path, Cursor::new(buffer as &mut Vec<u8>));

                let path_iter = NibbleIterator::new(buffer.iter().copied());
                let (root_node, old_value) =
                    root_node.insert(&mut self.nodes, &mut self.values, &path, path_iter, value);
                self.root_ref = self.nodes.insert(root_node);

                old_value
            }
            None => {
                // If the tree is empty, just add a leaf.
                let mut path_len = 0;
                let value_ref = self
                    .values
                    .insert(build_value::<V, H>(value, Some(&mut path_len)));
                let node_ref = self.nodes.insert(LeafNode::new(path_len, value_ref).into());
                assert_eq!(node_ref, 0, "inconsistent internal tree structure");

                None
            }
        }
    }

    pub fn remove(&mut self, path: &V::Path) -> Option<V> {
        todo!()
    }

    // TODO: Iterators.

    pub fn compute_hash(&self) -> Output<H> {
        todo!()
    }
}
