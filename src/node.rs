use crate::{
    nibble::Nibble,
    nodes::{BranchNode, ExtensionNode, LeafNode},
    util::Offseted,
    NodesStorage, TreePath, ValuesStorage,
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
    P: TreePath,
    V: AsRef<[u8]>,
    H: Digest,
{
    Branch(BranchNode<P, V, H>),
    Extension(ExtensionNode<P, V, H>),
    Leaf(LeafNode<P, V, H>),
}

impl<P, V, H> Node<P, V, H>
where
    P: TreePath,
    V: AsRef<[u8]>,
    H: Digest,
{
    pub fn get<'a, I>(
        &'a self,
        nodes: &'a NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V>,
        path_iter: Offseted<I>,
    ) -> Option<&V>
    where
        I: Iterator<Item = Nibble>,
    {
        match self {
            Node::Branch(branch_node) => branch_node.get(nodes, values, path_iter),
            Node::Extension(extension_node) => extension_node.get(nodes, values, path_iter),
            Node::Leaf(leaf_node) => leaf_node.get(nodes, values, path_iter),
        }
    }

    pub fn insert<I>(
        self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        path_iter: Offseted<I>,
    ) -> (Self, InsertAction)
    where
        I: Iterator<Item = Nibble>,
    {
        match self {
            Node::Branch(branch_node) => branch_node.insert(nodes, values, path_iter),
            Node::Extension(extension_node) => extension_node.insert(nodes, values, path_iter),
            Node::Leaf(leaf_node) => leaf_node.insert(nodes, values, path_iter),
        }
    }

    pub fn remove<I>(
        self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        path_iter: Offseted<I>,
    ) -> (Option<Self>, Option<V>)
    where
        I: Iterator<Item = Nibble>,
    {
        match self {
            Node::Branch(branch_node) => branch_node.remove(nodes, values, path_iter),
            Node::Extension(extension_node) => extension_node.remove(nodes, values, path_iter),
            Node::Leaf(leaf_node) => leaf_node.remove(nodes, values, path_iter),
        }
    }

    pub fn compute_hash(
        &mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &ValuesStorage<P, V>,
        key_offset: usize,
    ) -> &[u8] {
        match self {
            Node::Branch(branch_node) => branch_node.compute_hash(nodes, values, key_offset),
            Node::Extension(extension_node) => {
                extension_node.compute_hash(nodes, values, key_offset)
            }
            Node::Leaf(leaf_node) => leaf_node.compute_hash(nodes, values, key_offset),
        }
    }
}

impl<P, V, H> From<BranchNode<P, V, H>> for Node<P, V, H>
where
    P: TreePath,
    V: AsRef<[u8]>,
    H: Digest,
{
    fn from(value: BranchNode<P, V, H>) -> Self {
        Self::Branch(value)
    }
}

impl<P, V, H> From<ExtensionNode<P, V, H>> for Node<P, V, H>
where
    P: TreePath,
    V: AsRef<[u8]>,
    H: Digest,
{
    fn from(value: ExtensionNode<P, V, H>) -> Self {
        Self::Extension(value)
    }
}

impl<P, V, H> From<LeafNode<P, V, H>> for Node<P, V, H>
where
    P: TreePath,
    V: AsRef<[u8]>,
    H: Digest,
{
    fn from(value: LeafNode<P, V, H>) -> Self {
        Self::Leaf(value)
    }
}

/// Returned by .insert() to update the values' storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsertAction {
    // /// No action is required.
    // Nothing,
    /// An insertion is required. The argument points to a node.
    Insert(usize),
    /// A replacement is required. The argument points to a value.
    Replace(usize),

    /// Special insert where its node_ref is not known.
    InsertSelf,
}

impl InsertAction {
    /// Replace `Self::InsertSelf` with `Self::Insert(node_ref)`.
    pub fn quantize_self(self, node_ref: usize) -> Self {
        match self {
            Self::InsertSelf => Self::Insert(node_ref),
            _ => self,
        }
    }
}
