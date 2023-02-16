use super::BranchNode;
use crate::{
    hashing::{NodeHash, NodeHashRef, NodeHasher, PathKind},
    nibble::{NibbleSlice, NibbleVec},
    node::{InsertAction, Node},
    nodes::LeafNode,
    Encode, NodeRef, NodesStorage, ValuesStorage,
};
use digest::Digest;
use std::marker::PhantomData;

#[derive(Clone, Debug)]
pub struct ExtensionNode<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    pub(crate) prefix: NibbleVec,
    // The child node may only be a branch, but it's not included directly by value to avoid
    // inflating `Node`'s size too much.
    pub(crate) child_ref: NodeRef,

    hash: NodeHash<H>,
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> ExtensionNode<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    pub(crate) fn new(prefix: NibbleVec, child_ref: NodeRef) -> Self {
        Self {
            prefix,
            child_ref,
            hash: Default::default(),
            phantom: PhantomData,
        }
    }

    pub fn get<'a>(
        &self,
        nodes: &'a NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V>,
        mut path: NibbleSlice,
    ) -> Option<&'a V> {
        // If the path is prefixed by this node's prefix, delegate to its child.
        // Otherwise, no value is present.

        path.skip_prefix(&self.prefix)
            .then(|| {
                let child_node = nodes
                    .get(*self.child_ref)
                    .expect("inconsistent internal tree structure");

                child_node.get(nodes, values, path)
            })
            .flatten()
    }

    pub(crate) fn insert(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        mut path: NibbleSlice,
    ) -> (Node<P, V, H>, InsertAction) {
        // Possible flow paths (there are duplicates between different prefix lengths):
        //   extension { [0], child } -> branch { 0 => child } with_value !
        //   extension { [0], child } -> extension { [0], child }
        //   extension { [0, 1], child } -> branch { 0 => extension { [1], child } } with_value !
        //   extension { [0, 1], child } -> extension { [0], branch { 1 => child } with_value ! }
        //   extension { [0, 1], child } -> extension { [0, 1], child }
        //   extension { [0, 1, 2], child } -> branch { 0 => extension { [1, 2], child } } with_value !
        //   extension { [0, 1, 2], child } -> extension { [0], branch { 1 => extension { [2], child } } with_value ! }
        //   extension { [0, 1, 2], child } -> extension { [0, 1], branch { 2 => child } with_value ! }
        //   extension { [0, 1, 2], child } -> extension { [0, 1, 2], child }

        self.hash.mark_as_dirty();

        if path.skip_prefix(&self.prefix) {
            let child_node = nodes
                .try_remove(*self.child_ref)
                .expect("inconsistent internal tree structure");

            let (child_node, insert_action) = child_node.insert(nodes, values, path);
            self.child_ref = NodeRef::new(nodes.insert(child_node));

            let insert_action = insert_action.quantize_self(self.child_ref);
            (self.into(), insert_action)
        } else {
            let offset = path.clone().count_prefix_vec(&self.prefix);
            path.offset_add(offset);
            let (left_prefix, choice, right_prefix) = self.prefix.split_extract_at(offset);

            let left_prefix = (!left_prefix.is_empty()).then_some(left_prefix);
            let right_prefix = (!right_prefix.is_empty()).then_some(right_prefix);

            // Prefix right node (if any, child is self.child_ref).
            let right_prefix_node = right_prefix
                .map(|right_prefix| {
                    nodes.insert(ExtensionNode::new(right_prefix, self.child_ref).into())
                })
                .unwrap_or(*self.child_ref);

            // Branch node (child is prefix right or self.child_ref).
            let mut insert_node_ref = None;
            let branch_node = BranchNode::new({
                let mut choices = [Default::default(); 16];
                choices[choice as usize] = NodeRef::new(right_prefix_node);
                if let Some(c) = path.next() {
                    choices[c as usize] =
                        NodeRef::new(nodes.insert(LeafNode::new(Default::default()).into()));
                    insert_node_ref = Some(choices[c as usize]);
                }
                choices
            });

            // Prefix left node (if any, child is branch_node).
            match left_prefix {
                Some(left_prefix) => {
                    let branch_ref = NodeRef::new(nodes.insert(branch_node.into()));

                    (
                        ExtensionNode::new(left_prefix, branch_ref).into(),
                        InsertAction::Insert(insert_node_ref.unwrap_or(branch_ref)),
                    )
                }
                None => match insert_node_ref {
                    Some(child_ref) => (branch_node.into(), InsertAction::Insert(child_ref)),
                    None => (branch_node.into(), InsertAction::InsertSelf),
                },
            }
        }
    }

    pub fn remove(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        mut path: NibbleSlice,
    ) -> (Option<Node<P, V, H>>, Option<V>) {
        // Possible flow paths:
        //   - extension { a, branch { ... } } -> extension { a, branch { ... }}
        //   - extension { a, branch { ... } } -> extension { a + b, branch { ... }}
        //   - extension { a, branch { ... } } -> leaf { ... }

        if path.skip_prefix(&self.prefix) {
            let child_node = nodes
                .try_remove(*self.child_ref)
                .expect("inconsistent internal tree structure");

            let (child_node, old_value) = child_node.remove(nodes, values, path);
            if old_value.is_some() {
                self.hash.mark_as_dirty();
            }

            let node = child_node.map(|x| match x {
                Node::Branch(branch_node) => {
                    self.child_ref = NodeRef::new(nodes.insert(branch_node.into()));
                    self.into()
                }
                Node::Extension(extension_node) => {
                    self.prefix.extend(&extension_node.prefix);
                    self.into()
                }
                Node::Leaf(leaf_node) => leaf_node.into(),
            });

            (node, old_value)
        } else {
            (Some(self.into()), None)
        }
    }

    pub fn compute_hash(
        &self,
        nodes: &NodesStorage<P, V, H>,
        values: &ValuesStorage<P, V>,
        path_offset: usize,
    ) -> NodeHashRef<H> {
        self.hash.extract_ref().unwrap_or_else(|| {
            let child_node = nodes
                .get(*self.child_ref)
                .expect("inconsistent internal tree structure");

            let child_hash_ref =
                child_node.compute_hash(nodes, values, path_offset + self.prefix.len());

            compute_extension_hash(&self.hash, &self.prefix, child_hash_ref)
        })
    }
}

pub fn compute_extension_hash<'a, H>(
    hash: &'a NodeHash<H>,
    prefix: &NibbleVec,
    child_hash_ref: NodeHashRef<H>,
) -> NodeHashRef<'a, H>
where
    H: Digest,
{
    let prefix_len = NodeHasher::<H>::path_len(prefix.len());
    let child_len = match &child_hash_ref {
        NodeHashRef::Inline(x) => x.len(),
        NodeHashRef::Hashed(x) => NodeHasher::<H>::bytes_len(x.len(), x[0]),
    };

    let mut hasher = NodeHasher::new(hash);
    hasher.write_list_header(prefix_len + child_len);
    hasher.write_path_vec(prefix, PathKind::Extension);
    match child_hash_ref {
        NodeHashRef::Inline(x) => hasher.write_raw(&x),
        NodeHashRef::Hashed(x) => hasher.write_bytes(&x),
    }
    hasher.finalize()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{nibble::Nibble, pmt_node, pmt_state};
    use sha3::Keccak256;

    #[test]
    fn new() {
        let node =
            ExtensionNode::<Vec<u8>, Vec<u8>, Keccak256>::new(NibbleVec::new(), Default::default());

        assert_eq!(node.prefix.len(), 0);
        assert_eq!(node.child_ref, NodeRef::default());
    }

    #[test]
    fn get_some() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0], branch {
                0 => leaf { vec![0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x01] => vec![0x34, 0x56, 0x78, 0x9A] },
            } }
        };

        assert_eq!(
            node.get(&nodes, &values, NibbleSlice::new(&[0x00]))
                .map(Vec::as_slice),
            Some([0x12, 0x34, 0x56, 0x78].as_slice()),
        );
        assert_eq!(
            node.get(&nodes, &values, NibbleSlice::new(&[0x01]))
                .map(Vec::as_slice),
            Some([0x34, 0x56, 0x78, 0x9A].as_slice()),
        );
    }

    #[test]
    fn get_none() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0], branch {
                0 => leaf { vec![0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x01] => vec![0x34, 0x56, 0x78, 0x9A] },
            } }
        };

        assert_eq!(
            node.get(&nodes, &values, NibbleSlice::new(&[0x02]))
                .map(Vec::as_slice),
            None,
        );
    }

    #[test]
    fn insert_passthrough() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0], branch {
                0 => leaf { vec![0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x01] => vec![0x34, 0x56, 0x78, 0x9A] },
            } }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x02]));
        let node = match node {
            Node::Extension(x) => x,
            _ => panic!("expected an extension node"),
        };

        // TODO: Check children.
        assert!(node.prefix.iter().eq([Nibble::V0].into_iter()));
        assert_eq!(insert_action, InsertAction::Insert(NodeRef::new(2)));
    }

    #[test]
    fn insert_branch() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0], branch {
                0 => leaf { vec![0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x01] => vec![0x34, 0x56, 0x78, 0x9A] },
            } }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x10]));
        let _ = match node {
            Node::Branch(x) => x,
            _ => panic!("expected a branch node"),
        };

        // TODO: Check node and children.
        assert_eq!(insert_action, InsertAction::Insert(NodeRef::new(3)));
    }

    #[test]
    fn insert_branch_extension() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0, 0], branch {
                0 => leaf { vec![0x00, 0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x00, 0x10] => vec![0x34, 0x56, 0x78, 0x9A] },
            } }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x10]));
        let _ = match node {
            Node::Branch(x) => x,
            _ => panic!("expected a branch node"),
        };

        // TODO: Check node and children.
        assert_eq!(insert_action, InsertAction::Insert(NodeRef::new(4)));
    }

    #[test]
    fn insert_extension_branch() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0, 0], branch {
                0 => leaf { vec![0x00, 0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x00, 0x10] => vec![0x34, 0x56, 0x78, 0x9A] },
            } }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x01]));
        let _ = match node {
            Node::Extension(x) => x,
            _ => panic!("expected an extension node"),
        };

        // TODO: Check node and children.
        assert_eq!(insert_action, InsertAction::Insert(NodeRef::new(3)));
    }

    #[test]
    fn insert_extension_branch_extension() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0, 0], branch {
                0 => leaf { vec![0x00, 0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x00, 0x10] => vec![0x34, 0x56, 0x78, 0x9A] },
            } }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x01]));
        let _ = match node {
            Node::Extension(x) => x,
            _ => panic!("expected an extension node"),
        };

        // TODO: Check node and children.
        assert_eq!(insert_action, InsertAction::Insert(NodeRef::new(3)));
    }

    #[test]
    fn remove_none() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0], branch {
                0 => leaf { vec![0x00] => vec![0x00] },
                1 => leaf { vec![0x01] => vec![0x01] },
            } }
        };

        let (node, value) = node.remove(&mut nodes, &mut values, NibbleSlice::new(&[0x02]));

        assert!(matches!(node, Some(Node::Extension(_))));
        assert_eq!(value, None);
    }

    #[test]
    fn remove_into_leaf() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0], branch {
                0 => leaf { vec![0x00] => vec![0x00] },
                1 => leaf { vec![0x01] => vec![0x01] },
            } }
        };

        let (node, value) = node.remove(&mut nodes, &mut values, NibbleSlice::new(&[0x01]));

        assert!(matches!(node, Some(Node::Leaf(_))));
        assert_eq!(value, Some(vec![0x01]));
    }

    #[test]
    fn remove_into_extension() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0], branch {
                0 => leaf { vec![0x00] => vec![0x00] },
                1 => extension { [0], branch {
                    0 => leaf { vec![0x01, 0x00] => vec![0x01, 0x00] },
                    1 => leaf { vec![0x01, 0x01] => vec![0x01, 0x01] },
                } },
            } }
        };

        let (node, value) = node.remove(&mut nodes, &mut values, NibbleSlice::new(&[0x00]));

        assert!(matches!(node, Some(Node::Extension(_))));
        assert_eq!(value, Some(vec![0x00]));
    }

    #[test]
    fn compute_hash() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0, 0], branch {
                0 => leaf { vec![0x00, 0x00] => vec![0x12, 0x34] },
                1 => leaf { vec![0x00, 0x10] => vec![0x56, 0x78] },
            } }
        };

        let node_hash_ref = node.compute_hash(&nodes, &values, 0);
        assert_eq!(
            node_hash_ref.as_ref(),
            &[
                0xDD, 0x82, 0x00, 0x00, 0xD9, 0xC4, 0x30, 0x82, 0x12, 0x34, 0xC4, 0x30, 0x82, 0x56,
                0x78, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
                0x80, 0x80,
            ],
        );
    }

    #[test]
    fn compute_hash_long() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            extension { [0, 0], branch {
                0 => leaf { vec![0x00, 0x00] => vec![0x12, 0x34, 0x56, 0x78, 0x9A] },
                1 => leaf { vec![0x00, 0x10] => vec![0x34, 0x56, 0x78, 0x9A, 0xBC] },
            } }
        };

        let node_hash_ref = node.compute_hash(&nodes, &values, 0);
        assert_eq!(
            node_hash_ref.as_ref(),
            &[
                0xFA, 0xBA, 0x42, 0x79, 0xB3, 0x9B, 0xCD, 0xEB, 0x7C, 0x53, 0x0F, 0xD7, 0x6E, 0x5A,
                0xA3, 0x48, 0xD3, 0x30, 0x76, 0x26, 0x14, 0x84, 0x55, 0xA0, 0xAE, 0xFE, 0x0F, 0x52,
                0x89, 0x5F, 0x36, 0x06,
            ],
        );
    }
}
