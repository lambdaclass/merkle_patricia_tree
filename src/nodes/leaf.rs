use super::{BranchNode, ExtensionNode};
use crate::{
    nibble::NibbleSlice,
    node::{InsertAction, Node},
    util::{encode_path, write_list, write_slice, DigestBuf},
    NodeRef, NodesStorage, ValueRef, ValuesStorage,
};
use digest::{Digest, Output};
use std::{io::Cursor, marker::PhantomData};

#[derive(Clone, Debug)]
pub struct LeafNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    value_ref: ValueRef,

    hash: (usize, Output<H>),
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> LeafNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    pub(crate) fn new(value_ref: ValueRef) -> Self {
        Self {
            value_ref,
            hash: (0, Default::default()),
            phantom: PhantomData,
        }
    }

    pub(crate) fn update_value_ref(&mut self, new_value_ref: ValueRef) {
        self.value_ref = new_value_ref;
    }

    pub fn get<'a>(
        &self,
        _nodes: &NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V>,
        path: NibbleSlice,
    ) -> Option<&'a V> {
        // If the remaining path (and offset) matches with the value's path, return the value.
        // Otherwise, no value is present.

        let (value_path, value) = values
            .get(self.value_ref.0)
            .expect("inconsistent internal tree structure");

        path.cmp_rest(value_path.as_ref()).then_some(value)
    }

    pub(crate) fn insert(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        path: NibbleSlice,
    ) -> (Node<P, V, H>, InsertAction) {
        // Possible flow paths:
        //   leaf { key => value } -> leaf { key => value }
        //   leaf { key => value } -> branch { 0 => leaf { key => value }, 1 => leaf { key => value } }
        //   leaf { key => value } -> extension { [0], branch { 0 => leaf { key => value }, 1 => leaf { key => value } } }
        //   leaf { key => value } -> extension { [0], branch { 0 => leaf { key => value } } with_value leaf { key => value } }
        //   leaf { key => value } -> extension { [0], branch { 0 => leaf { key => value } } with_value leaf { key => value } } // leafs swapped

        self.hash.0 = 0;

        let (value_path, _) = values
            .get(self.value_ref.0)
            .expect("inconsistent internal tree structure");

        if path.cmp_rest(value_path.as_ref()) {
            let value_ref = self.value_ref;
            (self.into(), InsertAction::Replace(value_ref))
        } else {
            // TODO: Implement dedicated method (half-byte avoid iterators).
            let offset = NibbleSlice::new(value_path.as_ref())
                .skip(path.offset())
                .zip(path.clone())
                .take_while(|(a, b)| a == b)
                .count();

            let mut path_branch = path.clone();
            path_branch.offset_add(offset);

            let absolute_offset = path_branch.offset();
            let (branch_node, mut insert_action) = if offset == 2 * path.as_ref().len() {
                (
                    BranchNode::new({
                        let mut choices = [None; 16];
                        // TODO: Dedicated method.
                        choices[NibbleSlice::new(value_path.as_ref())
                            .nth(absolute_offset)
                            .unwrap() as usize] = Some(NodeRef(nodes.insert(self.into())));
                        choices
                    }),
                    InsertAction::InsertSelf,
                )
            } else if offset == 2 * value_path.as_ref().len() {
                let child_ref = nodes.insert(LeafNode::new(Default::default()).into());
                let mut branch_node = BranchNode::new({
                    let mut choices = [None; 16];
                    // TODO: Dedicated method.
                    choices[path_branch.next().unwrap() as usize] = Some(NodeRef(child_ref));
                    choices
                });
                branch_node.update_value_ref(Some(self.value_ref));

                (branch_node, InsertAction::Insert(NodeRef(child_ref)))
            } else {
                let child_ref = nodes.insert(LeafNode::new(Default::default()).into());

                (
                    BranchNode::new({
                        let mut choices = [None; 16];
                        // TODO: Dedicated method.
                        choices[NibbleSlice::new(value_path.as_ref())
                            .nth(absolute_offset)
                            .unwrap() as usize] = Some(NodeRef(nodes.insert(self.into())));
                        // TODO: Dedicated method.
                        choices[path_branch.next().unwrap() as usize] = Some(NodeRef(child_ref));
                        choices
                    }),
                    InsertAction::Insert(NodeRef(child_ref)),
                )
            };

            let final_node = if offset != 0 {
                let branch_ref = NodeRef(nodes.insert(branch_node.into()));
                insert_action = insert_action.quantize_self(branch_ref);

                ExtensionNode::new(path.split_to_vec(offset), branch_ref).into()
            } else {
                branch_node.into()
            };

            (final_node, insert_action)
        }
    }

    pub fn compute_hash(
        &mut self,
        _nodes: &mut NodesStorage<P, V, H>,
        values: &ValuesStorage<P, V>,
        key_offset: usize,
    ) -> &[u8] {
        if self.hash.0 == 0 {
            let (key, value) = values
                .get(self.value_ref.0)
                .expect("inconsistent internal tree structure");

            let mut digest_buf = DigestBuf::<H>::new();

            // Encode key.
            // TODO: Improve performance by avoiding allocations.
            let key: Vec<_> = NibbleSlice::new(key.as_ref()).skip(key_offset).collect();
            let key_buf = encode_path(&key);

            let mut payload = Cursor::new(Vec::new());
            write_slice(&key_buf, &mut payload);

            // Encode value.
            // TODO: Improve performance by avoiding allocations.
            write_slice(value.as_ref(), &mut payload);

            write_list(&payload.into_inner(), &mut digest_buf);
            self.hash.0 = digest_buf.extract_or_finalize(&mut self.hash.1);
        }

        &self.hash.1[..self.hash.0]
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{pmt_node, pmt_state, util::INVALID_REF};
    use sha3::Keccak256;

    #[test]
    fn new() {
        let node = LeafNode::<Vec<u8>, Vec<u8>, Keccak256>::new(Default::default());
        assert_eq!(node.value_ref, ValueRef(INVALID_REF));
    }

    #[test]
    fn get_some() {
        let (nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        assert_eq!(
            node.get(&nodes, &values, NibbleSlice::new(&[0x12]))
                .map(Vec::as_slice),
            Some([0x12, 0x34, 0x56, 0x78].as_slice()),
        );
    }

    #[test]
    fn get_none() {
        let (nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        assert_eq!(
            node.get(&nodes, &values, NibbleSlice::new(&[0x34]))
                .map(Vec::as_slice),
            None,
        );
    }

    #[test]
    fn insert_replace() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x12]));
        let node = match node {
            Node::Leaf(x) => x,
            _ => panic!("expected a leaf node"),
        };

        assert_eq!(node.value_ref, ValueRef(0));
        assert_eq!(node.hash.0, 0);
        assert_eq!(insert_action, InsertAction::Replace(ValueRef(0)));
    }

    #[test]
    fn insert_branch() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x22]));
        let _ = match node {
            Node::Branch(x) => x,
            _ => panic!("expected a branch node"),
        };

        // TODO: Check branch.
        assert_eq!(insert_action, InsertAction::Insert(NodeRef(0)));
    }

    #[test]
    fn insert_extension_branch() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x13]));
        let _ = match node {
            Node::Extension(x) => x,
            _ => panic!("expected an extension node"),
        };

        // TODO: Check extension (and child branch).
        assert_eq!(insert_action, InsertAction::Insert(NodeRef(0)));
    }

    #[test]
    fn insert_extension_branch_value_self() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        let (node, insert_action) =
            node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x12, 0x34]));
        let _ = match node {
            Node::Extension(x) => x,
            _ => panic!("expected an extension node"),
        };

        // TODO: Check extension (and children).
        assert_eq!(insert_action, InsertAction::Insert(NodeRef(0)));
    }

    #[test]
    fn insert_extension_branch_value_other() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12, 0x34] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x12]));
        let _ = match node {
            Node::Extension(x) => x,
            _ => panic!("expected an extension node"),
        };

        // TODO: Check extension (and children).
        assert_eq!(insert_action, InsertAction::Insert(NodeRef(1)));
    }

    // An insertion that returns branch [value=(x)] -> leaf (y) is not possible because of the key
    // restrictions: nibbles come in pairs. If the first nibble is different, the node will be a
    // branch but it cannot have a value. If the second nibble is different, then it'll be an
    // extension followed by a branch with value and a child.
    //
    // Because of that, the two tests that would check those cases are neither necessary nor
    // possible.
}
