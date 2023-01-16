use super::BranchNode;
use crate::{
    nibble::{NibbleSlice, NibbleVec},
    node::{InsertAction, Node},
    util::{encode_path, write_list, write_slice, DigestBuf},
    NodeRef, NodesStorage, ValuesStorage,
};
use digest::{Digest, Output};
use std::{io::Cursor, marker::PhantomData};

#[derive(Clone, Debug)]
pub struct ExtensionNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    prefix: NibbleVec,
    // The child node may only be a branch, but it's not included directly by value to avoid
    // inflating `Node`'s size too much.
    child_ref: NodeRef,

    hash: (usize, Output<H>),
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> ExtensionNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    pub(crate) fn new(prefix: NibbleVec, child_ref: NodeRef) -> Self {
        Self {
            prefix,
            child_ref,
            hash: (0, Default::default()),
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
                    .get(self.child_ref.0)
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

        self.hash.0 = 0;

        if path.skip_prefix(&self.prefix) {
            let child_node = nodes
                .try_remove(self.child_ref.0)
                .expect("inconsistent internal tree structure");

            let (child_node, insert_action) = child_node.insert(nodes, values, path);
            self.child_ref = NodeRef(nodes.insert(child_node));

            let insert_action = insert_action.quantize_self(self.child_ref);
            (self.into(), insert_action)
        } else {
            // TODO: Implement dedicated method (avoid half-byte iterators).
            let offset = self
                .prefix
                .iter()
                .zip(path.clone())
                .take_while(|(a, b)| a == b)
                .count();
            assert!(
                offset < self.prefix.iter().count(),
                "{:#02x?}, {:#02x?}",
                self.prefix,
                path
            );
            let (left_prefix, choice, right_prefix) = self.prefix.split_extract_at(offset);

            let left_prefix = (!left_prefix.is_empty()).then_some(left_prefix);
            let right_prefix = (!right_prefix.is_empty()).then_some(right_prefix);

            // Prefix right node (if any, child is self.child_ref).
            let right_prefix_node = right_prefix
                .map(|right_prefix| {
                    nodes.insert(ExtensionNode::new(right_prefix, self.child_ref).into())
                })
                .unwrap_or(self.child_ref.0);

            // Branch node (child is prefix right or self.child_ref).
            let branch_node = BranchNode::new({
                let mut choices = [None; 16];
                choices[choice as usize] = Some(NodeRef(right_prefix_node));
                choices
            })
            .into();

            // Prefix left node (if any, child is branch_node).
            match left_prefix {
                Some(left_prefix) => {
                    let branch_ref = NodeRef(nodes.insert(branch_node));

                    (
                        ExtensionNode::new(left_prefix, branch_ref).into(),
                        InsertAction::Insert(branch_ref),
                    )
                }
                None => (branch_node, InsertAction::InsertSelf),
            }
        }
    }

    pub fn compute_hash(
        &mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &ValuesStorage<P, V>,
        key_offset: usize,
    ) -> &[u8] {
        if self.hash.0 == 0 {
            let mut payload = Cursor::new(Vec::new());

            let mut digest_buf = DigestBuf::<H>::new();

            let prefix = encode_path(&self.prefix.iter().collect::<Vec<_>>());
            write_slice(&prefix, &mut payload);

            let mut child = nodes
                .try_remove(self.child_ref.0)
                .expect("inconsistent internal tree structure");
            let child_hash =
                child.compute_hash(nodes, values, key_offset + self.prefix.iter().count());
            write_slice(child_hash, &mut payload);

            write_list(&payload.into_inner(), &mut digest_buf);
            self.hash.0 = digest_buf.extract_or_finalize(&mut self.hash.1);
        }

        &self.hash.1
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{nibble::Nibble, pmt_node, pmt_state, util::INVALID_REF};
    use sha3::Keccak256;

    #[test]
    fn new() {
        let node =
            ExtensionNode::<Vec<u8>, Vec<u8>, Keccak256>::new(NibbleVec::new(), Default::default());

        assert_eq!(node.prefix.iter().count(), 0);
        assert_eq!(node.child_ref, NodeRef(INVALID_REF));
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
        assert_eq!(insert_action, InsertAction::Insert(NodeRef(2)));
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
        assert_eq!(insert_action, InsertAction::InsertSelf);
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
        assert_eq!(insert_action, InsertAction::InsertSelf);
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
        assert_eq!(insert_action, InsertAction::Insert(NodeRef(3)));
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
        assert_eq!(insert_action, InsertAction::Insert(NodeRef(3)));
    }
}
