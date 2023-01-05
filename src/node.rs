use crate::{
    nibble::NibbleIterator,
    nodes::{BranchNode, ExtensionNode, LeafNode},
    TreePath,
};
use digest::{Digest, Output};
use generic_array::ArrayLength;
use slab::Slab;

/// A node within the Patricia Merkle tree.
///
/// Notes:
///   - The empty node allows dropping `Option<Node<...>>` in favor of simply using `Node<...>`.
///   - The variants `Branch` and `Extension` both have a `Leaf` version, which is used when said
///     node is also a leaf.
///   - Extension nodes are only used when followed by a branch, and never with other extensions
///     (they are combined) or leaves (they are removed).
pub enum Node<V, H>
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
{
    Branch(BranchNode<V, H>),
    LeafBranch(BranchNode<V, H>, LeafNode<V, H>),

    Extension(ExtensionNode<V, H>),
    LeafExtension(ExtensionNode<V, H>, LeafNode<V, H>),

    Leaf(LeafNode<V, H>),
}

impl<V, H> Node<V, H>
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
{
    pub fn get<'a, I>(
        &'a self,
        nodes: &'a Slab<Node<V, H>>,
        values: &'a Slab<(<H::OutputSize as ArrayLength<u8>>::ArrayType, V)>,
        full_path: &V::Path,
        mut path_iter: NibbleIterator<I>,
    ) -> Option<&V>
    where
        I: Iterator<Item = u8>,
    {
        match self {
            Node::Branch(branch_node) => branch_node.get(nodes, values, full_path, path_iter),
            Node::LeafBranch(branch_node, leaf_node) => {
                if path_iter.is_done() {
                    leaf_node.get(nodes, values, full_path, path_iter)
                } else {
                    branch_node.get(nodes, values, full_path, path_iter)
                }
            }
            Node::Extension(extension_node) => {
                extension_node.get(nodes, values, full_path, path_iter)
            }
            Node::LeafExtension(extension_node, leaf_node) => {
                if path_iter.is_done() {
                    leaf_node.get(nodes, values, full_path, path_iter)
                } else {
                    extension_node.get(nodes, values, full_path, path_iter)
                }
            }
            Node::Leaf(leaf_node) => leaf_node.get(nodes, values, full_path, path_iter),
        }
    }

    pub fn insert<I>(
        self,
        nodes: &mut Slab<Node<V, H>>,
        values: &mut Slab<(<H::OutputSize as ArrayLength<u8>>::ArrayType, V)>,
        full_path: &V::Path,
        mut path_iter: NibbleIterator<I>,
        value: V,
    ) -> (Self, Option<V>)
    where
        I: Clone + Iterator<Item = u8>,
    {
        match self {
            Node::Branch(branch_node) => {
                branch_node.insert(nodes, values, full_path, path_iter, value)
            }
            Node::LeafBranch(branch_node, leaf_node) => {
                if path_iter.is_done() {
                    leaf_node.insert(nodes, values, full_path, path_iter, value)
                } else {
                    branch_node.insert(nodes, values, full_path, path_iter, value)
                }
            }
            Node::Extension(extension_node) => {
                extension_node.insert(nodes, values, full_path, path_iter, value)
            }
            Node::LeafExtension(extension_node, leaf_node) => {
                if path_iter.is_done() {
                    leaf_node.insert(nodes, values, full_path, path_iter, value)
                } else {
                    extension_node.insert(nodes, values, full_path, path_iter, value)
                }
            }
            Node::Leaf(leaf_node) => leaf_node.insert(nodes, values, full_path, path_iter, value),
        }
    }
}

impl<V, H> From<BranchNode<V, H>> for Node<V, H>
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
{
    fn from(value: BranchNode<V, H>) -> Self {
        Self::Branch(value)
    }
}

impl<V, H> From<(BranchNode<V, H>, LeafNode<V, H>)> for Node<V, H>
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
{
    fn from(value: (BranchNode<V, H>, LeafNode<V, H>)) -> Self {
        Self::LeafBranch(value.0, value.1)
    }
}

impl<V, H> From<ExtensionNode<V, H>> for Node<V, H>
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
{
    fn from(value: ExtensionNode<V, H>) -> Self {
        Self::Extension(value)
    }
}

impl<V, H> From<(ExtensionNode<V, H>, LeafNode<V, H>)> for Node<V, H>
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
{
    fn from(value: (ExtensionNode<V, H>, LeafNode<V, H>)) -> Self {
        Self::LeafExtension(value.0, value.1)
    }
}

impl<V, H> From<LeafNode<V, H>> for Node<V, H>
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
{
    fn from(value: LeafNode<V, H>) -> Self {
        Self::Leaf(value)
    }
}
