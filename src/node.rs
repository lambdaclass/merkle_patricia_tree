use crate::{
    hashing::NodeHashRef,
    nibble::NibbleSlice,
    nodes::{BranchNode, ExtensionNode, LeafNode},
    Encode, NodeRef, NodesStorage, ValueRef, ValuesStorage,
};
use digest::Digest;

/// A node within the Patricia Merkle tree.
///
/// Notes:
///   - The `Branch` variant havs an optional value.
///   - Extension nodes are only used when followed by a branch, and never with other extensions
///     (they are combined) or leaves (they are removed).
#[derive(Clone, Debug)]
pub enum Node<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    Branch(BranchNode<P, V, H>),
    Extension(ExtensionNode<P, V, H>),
    Leaf(LeafNode<P, V, H>),
}

impl<P, V, H> Node<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    pub fn get<'a>(
        &'a self,
        nodes: &'a NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V>,
        path: NibbleSlice,
    ) -> Option<&V> {
        match self {
            Node::Branch(branch_node) => branch_node.get(nodes, values, path),
            Node::Extension(extension_node) => extension_node.get(nodes, values, path),
            Node::Leaf(leaf_node) => leaf_node.get(nodes, values, path),
        }
    }

    pub(crate) fn insert(
        self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        path: NibbleSlice,
    ) -> (Self, InsertAction) {
        match self {
            Node::Branch(branch_node) => branch_node.insert(nodes, values, path),
            Node::Extension(extension_node) => extension_node.insert(nodes, values, path),
            Node::Leaf(leaf_node) => leaf_node.insert(nodes, values, path),
        }
    }

    pub(crate) fn remove(
        self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        path: NibbleSlice,
    ) -> (Option<Self>, Option<V>) {
        match self {
            Node::Branch(branch_node) => branch_node.remove(nodes, values, path),
            Node::Extension(extension_node) => extension_node.remove(nodes, values, path),
            Node::Leaf(leaf_node) => leaf_node.remove(nodes, values, path),
        }
    }

    pub fn compute_hash(
        &self,
        nodes: &NodesStorage<P, V, H>,
        values: &ValuesStorage<P, V>,
        path_offset: usize,
    ) -> NodeHashRef<H> {
        match self {
            Node::Branch(branch_node) => branch_node.compute_hash(nodes, values, path_offset),
            Node::Extension(extension_node) => {
                extension_node.compute_hash(nodes, values, path_offset)
            }
            Node::Leaf(leaf_node) => leaf_node.compute_hash(nodes, values, path_offset),
        }
    }
}

impl<P, V, H> From<BranchNode<P, V, H>> for Node<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    fn from(value: BranchNode<P, V, H>) -> Self {
        Self::Branch(value)
    }
}

impl<P, V, H> From<ExtensionNode<P, V, H>> for Node<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    fn from(value: ExtensionNode<P, V, H>) -> Self {
        Self::Extension(value)
    }
}

impl<P, V, H> From<LeafNode<P, V, H>> for Node<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    fn from(value: LeafNode<P, V, H>) -> Self {
        Self::Leaf(value)
    }
}

/// Returned by .insert() to update the values' storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InsertAction {
    /// An insertion is required. The argument points to a node.
    Insert(NodeRef),
    /// A replacement is required. The argument points to a value.
    Replace(ValueRef),

    /// Special insert where its node_ref is not known.
    InsertSelf,
}

impl InsertAction {
    /// Replace `Self::InsertSelf` with `Self::Insert(node_ref)`.
    pub const fn quantize_self(self, node_ref: NodeRef) -> Self {
        match self {
            Self::InsertSelf => Self::Insert(node_ref),
            _ => self,
        }
    }
}
