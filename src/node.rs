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
///   - The empty node allows dropping `Option<Node<...>>` in favor of simply using `Node<...>`.
///   - The variants `Branch` and `Extension` both have a `Leaf` version, which is used when said
///     node is also a leaf.
///   - Extension nodes are only used when followed by a branch, and never with other extensions
///     (they are combined) or leaves (they are removed).
#[derive(Clone, Debug)]
pub enum Node<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    Branch(BranchNode<P, V, H>),
    LeafBranch(BranchNode<P, V, H>, LeafNode<P, V, H>),

    Extension(ExtensionNode<P, V, H>),
    LeafExtension(ExtensionNode<P, V, H>, LeafNode<P, V, H>),

    Leaf(LeafNode<P, V, H>),
}

impl<P, V, H> Node<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    pub fn get<'a, I>(
        &'a self,
        nodes: &'a NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V, H>,
        mut path_iter: Offseted<I>,
    ) -> Option<&V>
    where
        I: Iterator<Item = Nibble>,
    {
        match self {
            Node::Branch(branch_node) => branch_node.get(nodes, values, path_iter),
            Node::LeafBranch(branch_node, leaf_node) => {
                if path_iter.peek().is_none() {
                    leaf_node.get(nodes, values, path_iter)
                } else {
                    branch_node.get(nodes, values, path_iter)
                }
            }
            Node::Extension(extension_node) => extension_node.get(nodes, values, path_iter),
            Node::LeafExtension(extension_node, leaf_node) => {
                if path_iter.peek().is_none() {
                    leaf_node.get(nodes, values, path_iter)
                } else {
                    extension_node.get(nodes, values, path_iter)
                }
            }
            Node::Leaf(leaf_node) => leaf_node.get(nodes, values, path_iter),
        }
    }

    pub fn insert<I>(
        self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V, H>,
        mut path_iter: Offseted<I>,
    ) -> (Self, InsertAction)
    where
        I: Iterator<Item = Nibble>,
    {
        match self {
            Node::Branch(branch_node) => branch_node.insert(nodes, values, path_iter),
            Node::LeafBranch(branch_node, leaf_node) => {
                if path_iter.peek().is_none() {
                    leaf_node.insert(nodes, values, path_iter)
                } else {
                    branch_node.insert(nodes, values, path_iter)
                }
            }
            Node::Extension(extension_node) => extension_node.insert(nodes, values, path_iter),
            Node::LeafExtension(extension_node, leaf_node) => {
                if path_iter.peek().is_none() {
                    leaf_node.insert(nodes, values, path_iter)
                } else {
                    extension_node.insert(nodes, values, path_iter)
                }
            }
            Node::Leaf(leaf_node) => leaf_node.insert(nodes, values, path_iter),
        }
    }
}

impl<P, V, H> From<BranchNode<P, V, H>> for Node<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    fn from(value: BranchNode<P, V, H>) -> Self {
        Self::Branch(value)
    }
}

impl<P, V, H> From<(BranchNode<P, V, H>, LeafNode<P, V, H>)> for Node<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    fn from(value: (BranchNode<P, V, H>, LeafNode<P, V, H>)) -> Self {
        Self::LeafBranch(value.0, value.1)
    }
}

impl<P, V, H> From<ExtensionNode<P, V, H>> for Node<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    fn from(value: ExtensionNode<P, V, H>) -> Self {
        Self::Extension(value)
    }
}

impl<P, V, H> From<(ExtensionNode<P, V, H>, LeafNode<P, V, H>)> for Node<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    fn from(value: (ExtensionNode<P, V, H>, LeafNode<P, V, H>)) -> Self {
        Self::LeafExtension(value.0, value.1)
    }
}

impl<P, V, H> From<LeafNode<P, V, H>> for Node<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    fn from(value: LeafNode<P, V, H>) -> Self {
        Self::Leaf(value)
    }
}

/// Returned by .insert() to update the values' storage.
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
