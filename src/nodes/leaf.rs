use super::{BranchNode, ExtensionNode};
use crate::{
    hashing::{NodeHash, NodeHashRef, NodeHasher, PathKind},
    nibble::NibbleSlice,
    node::{InsertAction, Node},
    Encode, NodeRef, NodesStorage, ValueRef, ValuesStorage,
};
use digest::Digest;
use std::marker::PhantomData;

#[derive(Clone, Debug)]
pub struct LeafNode<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    pub(crate) value_ref: ValueRef,

    hash: NodeHash<H>,
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> LeafNode<P, V, H>
where
    P: Encode,
    V: Encode,
    H: Digest,
{
    pub(crate) fn new(value_ref: ValueRef) -> Self {
        Self {
            value_ref,
            hash: Default::default(),
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
            .get(*self.value_ref)
            .expect("inconsistent internal tree structure");

        let encoded_value_path = value_path.encode();
        path.cmp_rest(encoded_value_path.as_ref()).then_some(value)
    }

    pub(crate) fn insert(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        path: NibbleSlice,
    ) -> (Node<P, V, H>, InsertAction) {
        // Possible flow paths:
        //   leaf { path => value } -> leaf { path => value }
        //   leaf { path => value } -> branch { 0 => leaf { path => value }, 1 => leaf { path => value } }
        //   leaf { path => value } -> extension { [0], branch { 0 => leaf { path => value }, 1 => leaf { path => value } } }
        //   leaf { path => value } -> extension { [0], branch { 0 => leaf { path => value } } with_value leaf { path => value } }
        //   leaf { path => value } -> extension { [0], branch { 0 => leaf { path => value } } with_value leaf { path => value } } // leafs swapped

        self.hash.mark_as_dirty();

        let (value_path, _) = values
            .get(*self.value_ref)
            .expect("inconsistent internal tree structure");

        let encoded_value_path = value_path.encode();
        if path.cmp_rest(encoded_value_path.as_ref()) {
            let value_ref = self.value_ref;
            (self.into(), InsertAction::Replace(value_ref))
        } else {
            let offset = path.clone().count_prefix_slice(&{
                let mut value_path = NibbleSlice::new(encoded_value_path.as_ref());
                value_path.offset_add(path.offset());
                value_path
            });

            let mut path_branch = path.clone();
            path_branch.offset_add(offset);

            let absolute_offset = path_branch.offset();
            let (branch_node, mut insert_action) = if absolute_offset == 2 * path.as_ref().len() {
                (
                    BranchNode::new({
                        let mut choices = [Default::default(); 16];
                        // TODO: Dedicated method.
                        choices[NibbleSlice::new(encoded_value_path.as_ref())
                            .nth(absolute_offset)
                            .unwrap() as usize] = NodeRef::new(nodes.insert(self.into()));
                        choices
                    }),
                    InsertAction::InsertSelf,
                )
            } else if absolute_offset == 2 * encoded_value_path.as_ref().len() {
                let child_ref = nodes.insert(LeafNode::new(Default::default()).into());
                let mut branch_node = BranchNode::new({
                    let mut choices = [Default::default(); 16];
                    choices[path_branch.next().unwrap() as usize] = NodeRef::new(child_ref);
                    choices
                });
                branch_node.update_value_ref(self.value_ref);

                (branch_node, InsertAction::Insert(NodeRef::new(child_ref)))
            } else {
                let child_ref = nodes.insert(LeafNode::new(Default::default()).into());

                (
                    BranchNode::new({
                        let mut choices = [Default::default(); 16];
                        // TODO: Dedicated method.
                        choices[NibbleSlice::new(encoded_value_path.as_ref())
                            .nth(absolute_offset)
                            .unwrap() as usize] = NodeRef::new(nodes.insert(self.into()));
                        choices[path_branch.next().unwrap() as usize] = NodeRef::new(child_ref);
                        choices
                    }),
                    InsertAction::Insert(NodeRef::new(child_ref)),
                )
            };

            let final_node = if offset != 0 {
                let branch_ref = NodeRef::new(nodes.insert(branch_node.into()));
                insert_action = insert_action.quantize_self(branch_ref);

                ExtensionNode::new(path.split_to_vec(offset), branch_ref).into()
            } else {
                branch_node.into()
            };

            (final_node, insert_action)
        }
    }

    pub(crate) fn remove(
        self,
        _nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        path: NibbleSlice,
    ) -> (Option<Node<P, V, H>>, Option<V>) {
        let (value_path, _) = values
            .get(*self.value_ref)
            .expect("inconsistent internal tree structure");

        let encoded_value_path = value_path.encode();
        if path.cmp_rest(encoded_value_path.as_ref()) {
            let (_, value) = values.remove(*self.value_ref);
            (None, Some(value))
        } else {
            (Some(self.into()), None)
        }
    }

    pub fn compute_hash(
        &self,
        _nodes: &NodesStorage<P, V, H>,
        values: &ValuesStorage<P, V>,
        path_offset: usize,
    ) -> NodeHashRef<H> {
        self.hash.extract_ref().unwrap_or_else(|| {
            let (path, value) = values
                .get(*self.value_ref)
                .expect("inconsistent internal tree structure");

            let encoded_path = path.encode();
            let encoded_value = value.encode();

            let mut path_slice = NibbleSlice::new(encoded_path.as_ref());
            path_slice.offset_add(path_offset);

            compute_leaf_hash(&self.hash, path_slice, encoded_value.as_ref())
        })
    }
}

pub fn compute_leaf_hash<'a, H>(
    hash: &'a NodeHash<H>,
    path: NibbleSlice,
    value: &[u8],
) -> NodeHashRef<'a, H>
where
    H: Digest,
{
    let path_len = NodeHasher::<H>::path_len(path.len());
    let value_len =
        NodeHasher::<H>::bytes_len(value.len(), value.first().copied().unwrap_or_default());

    let mut hasher = NodeHasher::new(hash);
    hasher.write_list_header(path_len + value_len);
    hasher.write_path_slice(&path, PathKind::Leaf);
    hasher.write_bytes(value);
    hasher.finalize()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{pmt_node, pmt_state};
    use sha3::Keccak256;

    #[test]
    fn new() {
        let node = LeafNode::<Vec<u8>, Vec<u8>, Keccak256>::new(Default::default());
        assert_eq!(node.value_ref, ValueRef::default());
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

        assert_eq!(node.value_ref, ValueRef::new(0));
        assert!(node.hash.extract_ref().is_none());
        assert_eq!(insert_action, InsertAction::Replace(ValueRef::new(0)));
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
        assert_eq!(insert_action, InsertAction::Insert(NodeRef::new(0)));
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
        assert_eq!(insert_action, InsertAction::Insert(NodeRef::new(0)));
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
        assert_eq!(insert_action, InsertAction::Insert(NodeRef::new(0)));
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
        assert_eq!(insert_action, InsertAction::Insert(NodeRef::new(1)));
    }

    // An insertion that returns branch [value=(x)] -> leaf (y) is not possible because of the path
    // restrictions: nibbles come in pairs. If the first nibble is different, the node will be a
    // branch but it cannot have a value. If the second nibble is different, then it'll be an
    // extension followed by a branch with value and a child.
    //
    // Because of that, the two tests that would check those cases are neither necessary nor
    // possible.

    #[test]
    fn remove_self() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12, 0x34] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        let (node, value) = node.remove(&mut nodes, &mut values, NibbleSlice::new(&[0x12, 0x34]));

        assert!(node.is_none());
        assert_eq!(value, Some(vec![0x12, 0x34, 0x56, 0x78]));
    }

    #[test]
    fn remove_none() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12, 0x34] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        let (node, value) = node.remove(&mut nodes, &mut values, NibbleSlice::new(&[0x12]));

        assert!(node.is_some());
        assert_eq!(value, None);
    }

    #[test]
    fn compute_hash() {
        let (nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { b"key".to_vec() => b"value".to_vec() }
        };

        let node_hash_ref = node.compute_hash(&nodes, &values, 0);
        assert_eq!(
            node_hash_ref.as_ref(),
            &[0xCB, 0x84, 0x20, 0x6B, 0x65, 0x79, 0x85, 0x76, 0x61, 0x6C, 0x75, 0x65],
        );
    }

    #[test]
    fn compute_hash_long() {
        let (nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { b"key".to_vec() => b"a comparatively long value".to_vec() }
        };

        let node_hash_ref = node.compute_hash(&nodes, &values, 0);
        assert_eq!(
            node_hash_ref.as_ref(),
            &[
                0xEB, 0x92, 0x75, 0xB3, 0xAE, 0x09, 0x3A, 0x17, 0x75, 0x7C, 0xFB, 0x42, 0xF7, 0xD5,
                0x57, 0xF9, 0xE5, 0x77, 0xBD, 0x5B, 0xEB, 0x86, 0xA8, 0x68, 0x49, 0x91, 0xA6, 0x5B,
                0x87, 0x5F, 0x80, 0x7A,
            ],
        );
    }
}
